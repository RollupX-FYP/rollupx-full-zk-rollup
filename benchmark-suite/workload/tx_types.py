"""
tx_types.py — Transaction type factories for RollupX benchmark workload.

Types differ in calldata size, gas profile, and DA footprint to
meaningfully stress the sequencer, prover, and DA layer.

Type A  Light transfer        ~100 B calldata   21,000 gas
Type B  Medium ERC-20 swap   ~300 B calldata   65,000 gas
Type C  Heavy contract call  ~600 B calldata  200,000 gas
"""

import os
import random
import time

try:
    from eth_account import Account
    from eth_account.messages import encode_defunct
except ImportError:
    raise ImportError("Run: pip install eth-account")

# ── sentinel target addresses ─────────────────────────────────────────────────
TYPE_TO_ADDR: dict[str, str] = {
    "A": "0x" + "02" * 20,
    "B": "0x" + "03" * 20,
    "C": "0x" + "04" * 20,
}

TYPE_GAS_LIMIT: dict[str, int] = {
    "A": 21_000,
    "B": 65_000,
    "C": 200_000,
}

# Gas price tiers (wei).  Higher class = higher priority in FeePriority policy.
TYPE_GAS_PRICE: dict[str, int] = {
    "A": 1_000_000_000,    # 1 gwei
    "B": 2_000_000_000,    # 2 gwei
    "C": 3_000_000_000,    # 3 gwei
}

# Calldata payload sizes (bytes of zero-padding appended to mimic real payloads)
TYPE_CALLDATA_EXTRA: dict[str, int] = {
    "A": 0,     # ~100 B total after headers
    "B": 200,   # ~300 B total
    "C": 500,   # ~600 B total
}

# Mix presets: (frac_A, frac_B, frac_C)
MIX_PRESETS: dict[str, tuple[float, float, float]] = {
    "balanced": (0.70, 0.20, 0.10),
    "light":    (0.95, 0.04, 0.01),
    "heavy":    (0.20, 0.30, 0.50),
}

# Hardhat default account — used in dev_mode
_DEFAULT_PRIVATE_KEY = (
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
)


def _sign_dummy(acct, nonce: int) -> str:
    """Return a flat 65-byte ECDSA signature hex for a dummy message."""
    msg = encode_defunct(text=f"rollupx_tx_{nonce}")
    signed = acct.sign_message(msg)
    r = hex(signed.r)[2:].zfill(64)
    s = hex(signed.s)[2:].zfill(64)
    v = hex(signed.v)[2:].zfill(2)
    return "0x" + r + s + v


def _extra_data(size: int) -> str:
    """Return a hex string of `size` zero bytes to pad calldata."""
    return "0x" + "00" * size if size > 0 else "0x"


class TxFactory:
    """
    Build benchmark transactions for Types A, B, and C.

    Parameters
    ----------
    private_key : str
        ECDSA private key for signing (default: Hardhat account #0).
    rng : random.Random | None
        Seeded RNG for reproducible value amounts.
    """

    def __init__(
        self,
        private_key: str = _DEFAULT_PRIVATE_KEY,
        seed: int | None = None,
    ):
        self.acct = Account.from_key(private_key)
        self.from_addr = self.acct.address
        self._rng = random.Random(seed)  # private, only used for value amounts

    def make(self, tx_type: str, nonce: int) -> dict:
        """
        Build a transaction dict ready to POST to /tx.

        Parameters
        ----------
        tx_type : "A" | "B" | "C"
        nonce   : sequential nonce

        Returns
        -------
        dict with keys: from, to, value, nonce, gas_price,
                        gas_limit, signature, timestamp, tx_type, calldata
        """
        if tx_type not in TYPE_TO_ADDR:
            raise ValueError(f"Unknown tx_type '{tx_type}'. Choose A, B, or C.")

        value = self._rng.randint(1, 1000)
        sig   = _sign_dummy(self.acct, nonce)

        return {
            "from":      self.from_addr,
            "to":        TYPE_TO_ADDR[tx_type],
            "value":     hex(value),
            "nonce":     nonce,
            "gas_price": hex(TYPE_GAS_PRICE[tx_type]),
            "gas_limit": hex(TYPE_GAS_LIMIT[tx_type]),
            "signature": sig,
            "timestamp": int(time.time()),
            "tx_type":   tx_type,
            # calldata simulates realistic payload size for proving/DA stress
            "calldata":  _extra_data(TYPE_CALLDATA_EXTRA[tx_type]),
        }

    # ── convenience batch builder ─────────────────────────────────────────────

    def make_batch(
        self,
        count: int,
        start_nonce: int,
        mix: str | tuple[float, float, float] = "balanced",
    ) -> list[dict]:
        """
        Build `count` transactions sampled from a type distribution.

        Parameters
        ----------
        count       : number of transactions
        start_nonce : starting nonce value
        mix         : preset name or (frac_A, frac_B, frac_C) tuple
        """
        fracs = MIX_PRESETS[mix] if isinstance(mix, str) else mix
        if abs(sum(fracs) - 1.0) > 1e-6:
            raise ValueError("Mix fractions must sum to 1.0")

        types = self._rng.choices(["A", "B", "C"], weights=fracs, k=count)
        return [self.make(t, start_nonce + i) for i, t in enumerate(types)]


def resolve_mix(
    preset: str | None,
    mix_a: float | None,
    mix_b: float | None,
    mix_c: float | None,
) -> tuple[float, float, float]:
    """
    Resolve tx mix from either a preset name or explicit fractions.
    Used by poisson_generator CLI.
    """
    if preset is not None:
        if preset not in MIX_PRESETS:
            raise ValueError(
                f"Unknown preset '{preset}'. Choose: {list(MIX_PRESETS)}"
            )
        return MIX_PRESETS[preset]

    if None in (mix_a, mix_b, mix_c):
        raise ValueError(
            "Provide --tx_mix preset OR all three of --mix_a, --mix_b, --mix_c"
        )

    total = mix_a + mix_b + mix_c
    if abs(total - 1.0) > 1e-4:
        raise ValueError(f"--mix_a + --mix_b + --mix_c must sum to 1.0, got {total}")
    return (mix_a, mix_b, mix_c)


# ── quick self-test ───────────────────────────────────────────────────────────
if __name__ == "__main__":
    import json

    factory = TxFactory(seed=42)

    for tx_type in ["A", "B", "C"]:
        tx = factory.make(tx_type, nonce=0)
        print(f"\nType {tx_type}:")
        print(json.dumps(tx, indent=2))

    print("\nBatch of 5 (balanced mix):")
    batch = factory.make_batch(5, start_nonce=10, mix="balanced")
    for tx in batch:
        print(f"  nonce={tx['nonce']}  type={tx['tx_type']}  "
              f"gas_limit={int(tx['gas_limit'], 16):,}  "
              f"calldata_bytes={len(bytes.fromhex(tx['calldata'][2:]))}")
