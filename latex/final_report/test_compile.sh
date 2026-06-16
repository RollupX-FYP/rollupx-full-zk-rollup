#!/bin/bash
set -e
pdflatex final_report_main.tex
bibtex final_report_main.aux
pdflatex final_report_main.tex
pdflatex final_report_main.tex
echo "Compilation successful."
