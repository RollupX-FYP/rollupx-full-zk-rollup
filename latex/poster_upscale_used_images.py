#!/usr/bin/env python3
"""
Deadline-safe poster image upscaler for LaTeX projects.

Run from the project root:
    python3 poster_upscale_used_images.py

What it does:
- Scans all .tex files for \includegraphics{...}
- Processes only used raster images under poster/images/
- Skips vector files, missing files, and unused images
- Creates a backup of poster/images
- Writes upscaled outputs to poster/images/upscaled/
- Updates only the includegraphics paths for successful outputs
- Attempts to compile with latexmk
- Writes a markdown report
"""
from __future__ import annotations

import argparse
import datetime as _dt
import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Tuple

RASTER_EXTS = {".png", ".jpg", ".jpeg", ".webp"}
VECTOR_EXTS = {".pdf", ".svg", ".eps"}
ALL_IMAGE_EXTS = RASTER_EXTS | VECTOR_EXTS

INCLUDE_RE = re.compile(
    r"(?P<cmd>\\includegraphics\s*(?:\[[^\]]*\]\s*)?\{)(?P<path>[^}]+)(?P<end>\})",
    flags=re.MULTILINE,
)

TEXT_HEAVY_8X_HINTS = {
    "abstract", "abstarct", "background", "architecture", "flow", "process",
    "story", "panel", "explanation", "method", "methodology", "dynamic",
    "batch", "costmodel", "cost_model", "pipeline", "system", "overview",
}

@dataclass
class ImageUse:
    raw_path: str
    resolved: Optional[Path]
    tex_files: List[Path] = field(default_factory=list)
    reason: str = ""

@dataclass
class UpscaleResult:
    src: Path
    dst: Optional[Path]
    scale: Optional[str]
    status: str
    note: str = ""


def run(cmd: List[str], cwd: Optional[Path] = None, check: bool = False) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=str(cwd) if cwd else None, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=check)


def which(name: str) -> Optional[str]:
    return shutil.which(name)


def rel_posix(path: Path, base: Path) -> str:
    return os.path.relpath(path, base).replace(os.sep, "/")


def is_under(path: Path, parent: Path) -> bool:
    try:
        path.resolve().relative_to(parent.resolve())
        return True
    except Exception:
        return False


def find_tex_files(root: Path) -> List[Path]:
    ignored_parts = {".git", "node_modules", "venv", ".venv", "__pycache__"}
    files = []
    for p in root.rglob("*.tex"):
        if any(part in ignored_parts for part in p.parts):
            continue
        files.append(p)
    return sorted(files)


def candidate_paths(raw: str, tex_file: Path, root: Path) -> Iterable[Path]:
    raw = raw.strip().strip('"').strip("'")
    p = Path(raw)

    # Direct path with extension.
    bases = []
    if p.is_absolute():
        bases.append(p)
    else:
        bases.append((tex_file.parent / p))
        bases.append((root / p))

    if p.suffix:
        for b in bases:
            yield b
    else:
        for b in bases:
            for ext in ALL_IMAGE_EXTS:
                yield Path(str(b) + ext)


def resolve_image(raw: str, tex_file: Path, root: Path) -> Optional[Path]:
    for c in candidate_paths(raw, tex_file, root):
        if c.exists():
            return c.resolve()
    return None


def scan_includegraphics(root: Path, tex_files: List[Path]) -> Dict[str, ImageUse]:
    found: Dict[str, ImageUse] = {}
    for tex in tex_files:
        try:
            text = tex.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            text = tex.read_text(errors="ignore")
        for m in INCLUDE_RE.finditer(text):
            raw = m.group("path").strip()
            resolved = resolve_image(raw, tex, root)
            key = raw if resolved is None else str(resolved)
            if key not in found:
                found[key] = ImageUse(raw_path=raw, resolved=resolved)
            found[key].tex_files.append(tex)
    return found


def get_dimensions(path: Path) -> Optional[Tuple[int, int]]:
    magick = which("magick") or which("convert")
    if not magick:
        return None
    try:
        if Path(magick).name == "magick":
            cmd = [magick, "identify", "-format", "%w %h", str(path)]
        else:
            cmd = [magick, "-format", "%w %h", str(path)]
        cp = run(cmd)
        if cp.returncode == 0:
            w, h = cp.stdout.strip().split()[:2]
            return int(w), int(h)
    except Exception:
        return None
    return None


def choose_scale(src: Path) -> str:
    name = src.stem.lower().replace("-", "_")
    dims = get_dimensions(src)
    if dims:
        w, h = dims
        area = w * h
        # Conservative 8x: use only for genuinely large/full-width or known section panels.
        if w >= 1200 or h >= 800 or area >= 900_000:
            return "8x"
    if any(hint in name for hint in TEXT_HEAVY_8X_HINTS):
        return "8x"
    return "4x"


def copy_backup(images_dir: Path, root: Path) -> Optional[Path]:
    if not images_dir.exists():
        return None
    stamp = _dt.datetime.now().strftime("%Y%m%d_%H%M%S")
    backup = images_dir.parent / f"images_backup_{stamp}"
    def ignore(dirpath: str, names: List[str]) -> set:
        # Do not recursively copy previous upscaled outputs or earlier backups into the backup.
        return {n for n in names if n == "upscaled" or n.startswith("images_backup_")}
    shutil.copytree(images_dir, backup, ignore=ignore)
    return backup


def upscale_image(src: Path, out_dir: Path, scale: str, model: str, allow_lanczos_fallback: bool) -> UpscaleResult:
    out_dir.mkdir(parents=True, exist_ok=True)
    safe_stem = src.stem
    dst = out_dir / f"{safe_stem}_{scale}.png"
    temp4 = out_dir / f".{safe_stem}_tmp_4x.png"

    realesrgan = which("realesrgan-ncnn-vulkan")
    magick = which("magick") or which("convert")

    try:
        if realesrgan:
            if scale == "4x":
                cmd = [realesrgan, "-i", str(src), "-o", str(dst), "-s", "4", "-n", model]
                cp = run(cmd)
                if cp.returncode != 0:
                    return UpscaleResult(src, None, scale, "failed", f"Real-ESRGAN failed: {cp.stdout[-1200:]}")
                return UpscaleResult(src, dst, scale, "upscaled", f"Real-ESRGAN {model}")

            # 8x = Real-ESRGAN 4x + Lanczos 2x + light sharpen.
            cmd = [realesrgan, "-i", str(src), "-o", str(temp4), "-s", "4", "-n", model]
            cp = run(cmd)
            if cp.returncode != 0:
                return UpscaleResult(src, None, scale, "failed", f"Real-ESRGAN 4x failed: {cp.stdout[-1200:]}")
            if not magick:
                return UpscaleResult(src, None, scale, "failed", "ImageMagick missing, cannot create 8x final from 4x output")
            if Path(magick).name == "magick":
                cmd2 = [magick, str(temp4), "-filter", "Lanczos", "-resize", "200%", "-unsharp", "0x1.0+0.6+0.02", str(dst)]
            else:
                cmd2 = [magick, str(temp4), "-filter", "Lanczos", "-resize", "200%", "-unsharp", "0x1.0+0.6+0.02", str(dst)]
            cp2 = run(cmd2)
            if cp2.returncode != 0:
                return UpscaleResult(src, None, scale, "failed", f"ImageMagick 8x step failed: {cp2.stdout[-1200:]}")
            try:
                temp4.unlink()
            except Exception:
                pass
            return UpscaleResult(src, dst, scale, "upscaled", f"Real-ESRGAN {model} 4x + Lanczos 2x")

        if not allow_lanczos_fallback:
            return UpscaleResult(src, None, scale, "skipped", "Real-ESRGAN not installed")
        if not magick:
            return UpscaleResult(src, None, scale, "failed", "Neither Real-ESRGAN nor ImageMagick is installed")

        # Deadline-safe local fallback: no AI, but smoother print output.
        percent = "400%" if scale == "4x" else "800%"
        unsharp = "0x0.8+0.45+0.02" if scale == "4x" else "0x1.0+0.6+0.02"
        if Path(magick).name == "magick":
            cmd = [magick, str(src), "-filter", "Lanczos", "-resize", percent, "-unsharp", unsharp, str(dst)]
        else:
            cmd = [magick, str(src), "-filter", "Lanczos", "-resize", percent, "-unsharp", unsharp, str(dst)]
        cp = run(cmd)
        if cp.returncode != 0:
            return UpscaleResult(src, None, scale, "failed", f"ImageMagick fallback failed: {cp.stdout[-1200:]}")
        return UpscaleResult(src, dst, scale, "upscaled", "ImageMagick Lanczos fallback only; Real-ESRGAN not installed")
    finally:
        if temp4.exists():
            try:
                temp4.unlink()
            except Exception:
                pass


def update_tex_paths(tex_files: List[Path], replacements: Dict[Path, Path], root: Path) -> List[Path]:
    updated = []
    for tex in tex_files:
        try:
            text = tex.read_text(encoding="utf-8")
            encoding = "utf-8"
        except UnicodeDecodeError:
            text = tex.read_text(errors="ignore")
            encoding = "utf-8"

        changed = False

        def repl(m: re.Match) -> str:
            nonlocal changed
            raw = m.group("path").strip()
            resolved = resolve_image(raw, tex, root)
            if resolved and resolved in replacements:
                new_rel = rel_posix(replacements[resolved], tex.parent)
                changed = True
                return f"{m.group('cmd')}{new_rel}{m.group('end')}"
            return m.group(0)

        new_text = INCLUDE_RE.sub(repl, text)
        if changed:
            tex.write_text(new_text, encoding=encoding)
            updated.append(tex)
    return updated


def find_main_tex(tex_files: List[Path], root: Path) -> Optional[Path]:
    candidates = []
    for tex in tex_files:
        try:
            text = tex.read_text(encoding="utf-8", errors="ignore")
        except TypeError:
            text = tex.read_text(errors="ignore")
        if "\\documentclass" in text:
            score = 0
            if tex.parent == root:
                score += 10
            if tex.name.lower() in {"main.tex", "poster.tex"}:
                score += 20
            candidates.append((score, tex))
    if not candidates:
        return None
    return sorted(candidates, reverse=True)[0][1]


def compile_pdf(main_tex: Optional[Path]) -> Tuple[str, str]:
    if main_tex is None:
        return "skipped", "No main .tex with \\documentclass found."
    latexmk = which("latexmk")
    pdflatex = which("pdflatex")
    if latexmk:
        cmd = [latexmk, "-pdf", "-interaction=nonstopmode", "-halt-on-error", main_tex.name]
    elif pdflatex:
        cmd = [pdflatex, "-interaction=nonstopmode", "-halt-on-error", main_tex.name]
    else:
        return "skipped", "No latexmk or pdflatex found."
    cp = run(cmd, cwd=main_tex.parent)
    if cp.returncode == 0:
        pdf = main_tex.with_suffix(".pdf")
        return "success", f"Compiled {pdf}" if pdf.exists() else "Compilation command succeeded."
    return "failed", cp.stdout[-3000:]


def write_report(
    root: Path,
    image_uses: Dict[str, ImageUse],
    results: List[UpscaleResult],
    skipped: List[Tuple[str, str]],
    updated_tex: List[Path],
    backup: Optional[Path],
    compile_status: Tuple[str, str],
    report_path: Path,
) -> None:
    lines = []
    lines.append("# Poster image upscaling report")
    lines.append("")
    lines.append(f"Project root: `{root}`")
    lines.append(f"Backup: `{backup}`" if backup else "Backup: not created; `poster/images` was not found")
    lines.append("")

    lines.append("## Images found in LaTeX")
    if image_uses:
        for use in image_uses.values():
            resolved = rel_posix(use.resolved, root) if use.resolved else "MISSING"
            texs = ", ".join(rel_posix(t, root) for t in sorted(set(use.tex_files)))
            lines.append(f"- `{use.raw_path}` → `{resolved}` in {texs}")
    else:
        lines.append("- None")
    lines.append("")

    lines.append("## Images upscaled")
    done = [r for r in results if r.status == "upscaled" and r.dst]
    if done:
        for r in done:
            lines.append(f"- `{rel_posix(r.src, root)}` → `{rel_posix(r.dst, root)}` — {r.scale} — {r.note}")
    else:
        lines.append("- None")
    lines.append("")

    lines.append("## Images skipped / failed")
    for raw, reason in skipped:
        lines.append(f"- `{raw}` — {reason}")
    for r in results:
        if r.status != "upscaled":
            lines.append(f"- `{rel_posix(r.src, root)}` — {r.status}: {r.note}")
    if not skipped and all(r.status == "upscaled" for r in results):
        lines.append("- None")
    lines.append("")

    lines.append("## LaTeX files updated")
    if updated_tex:
        for t in updated_tex:
            lines.append(f"- `{rel_posix(t, root)}`")
    else:
        lines.append("- None")
    lines.append("")

    lines.append("## Final PDF compile status")
    lines.append(f"- {compile_status[0]}: {compile_status[1]}")
    lines.append("")
    lines.append("## Reverts")
    lines.append("- None performed automatically. If a specific upscaled image looks worse at 400% zoom, manually revert only that `\\includegraphics` path to the original.")
    lines.append("")

    report_path.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("root", nargs="?", default=".", help="Project root; default: current directory")
    ap.add_argument("--model", default="realesrgan-x4plus", help="Real-ESRGAN model name; default: realesrgan-x4plus")
    ap.add_argument("--no-lanczos-fallback", action="store_true", help="Skip images if Real-ESRGAN is not installed")
    ap.add_argument("--dry-run", action="store_true", help="Scan and report only; do not backup, upscale, update, or compile")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    images_dir = root / "poster" / "images"
    upscaled_dir = images_dir / "upscaled"
    report_path = root / "poster_image_upscale_report.md"

    tex_files = find_tex_files(root)
    image_uses = scan_includegraphics(root, tex_files)

    skipped: List[Tuple[str, str]] = []
    to_process: Dict[Path, str] = {}

    for use in image_uses.values():
        raw = use.raw_path
        resolved = use.resolved
        if resolved is None:
            skipped.append((raw, "missing path"))
            continue
        ext = resolved.suffix.lower()
        if ext in VECTOR_EXTS:
            skipped.append((raw, "vector file skipped"))
            continue
        if ext not in RASTER_EXTS:
            skipped.append((raw, "unsupported/non-raster image type"))
            continue
        if not is_under(resolved, images_dir):
            skipped.append((raw, "raster image is not inside poster/images/"))
            continue
        if is_under(resolved, upscaled_dir):
            skipped.append((raw, "already inside poster/images/upscaled/"))
            continue
        to_process[resolved] = choose_scale(resolved)

    backup = None
    results: List[UpscaleResult] = []
    updated_tex: List[Path] = []
    compile_status = ("skipped", "dry run")

    if not args.dry_run:
        if to_process:
            backup = copy_backup(images_dir, root)
            upscaled_dir.mkdir(parents=True, exist_ok=True)
            for src, scale in sorted(to_process.items()):
                results.append(upscale_image(src, upscaled_dir, scale, args.model, not args.no_lanczos_fallback))
            replacements = {r.src: r.dst for r in results if r.status == "upscaled" and r.dst}
            updated_tex = update_tex_paths(tex_files, replacements, root)
        main_tex = find_main_tex(tex_files, root)
        compile_status = compile_pdf(main_tex)

    write_report(root, image_uses, results, skipped, updated_tex, backup, compile_status, report_path)

    print(f"Report written: {report_path}")
    if results:
        for r in results:
            out = rel_posix(r.dst, root) if r.dst else "-"
            print(f"{r.status}: {rel_posix(r.src, root)} -> {out} ({r.scale}) {r.note}")
    elif not to_process:
        print("No eligible used raster images found under poster/images/.")
    print(f"Compile: {compile_status[0]} - {compile_status[1].splitlines()[0] if compile_status[1] else ''}")
    return 0 if compile_status[0] != "failed" else 2

if __name__ == "__main__":
    raise SystemExit(main())
