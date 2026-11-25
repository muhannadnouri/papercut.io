"""Regenerate the DS-004 PDF with ReportLab. Smoke runs use the checked-in copy."""
from pathlib import Path
from reportlab.pdfgen.canvas import Canvas

title = "Estuary Survey"
pdf = Canvas(str(Path(__file__).with_name(title + ".pdf")), pagesize=(600, 1200), invariant=1)
pdf.setTitle(title)
for page in range(1, 8):
    pdf.drawString(42, 1140, f"Estuary survey station {page}")
    pdf.drawString(42, 150, "Copper buoys mark the inlet." if page == 6 else f"End of station {page}.")
    pdf.showPage()
pdf.save()
