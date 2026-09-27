#!/bin/sh
# Makes the sample files the content tests read, with the Mac's own tools.
# Run from this folder: sh make.sh. The files are committed; rerun only to change them.
set -e
cd "$(dirname "$0")"

printf 'Receipt from Blue Bottle Coffee\nCafé latte 4.50\nTotal 4.50\n' > note.txt
printf 'Caf\351 cr\350me 3.20\n' > latin1.txt
printf '# Meeting notes\n\n- Budget for **2026** agreed\n' > notes.md
printf 'Date,Vendor,Amount\n2026-09-14,Blue Bottle,4.50\n' > expenses.csv
printf '{"vendor": "Blue Bottle", "total": 4.5}\n' > order.json
printf '1\n00:00:01,000 --> 00:00:03,000\nWelcome to the course\n' > lecture.srt
printf 'WEBVTT\n\n00:00.000 --> 00:02.000\nWelcome back\n' > lecture.vtt
cat > page.html <<'HTML'
<!doctype html><html><head><title>Invoice</title><style>body { color: red }</style>
<script>var secret = "not text";</script></head>
<body><h1>Invoice 4471</h1><p>Total &amp; tax: 12&nbsp;EUR &mdash; paid</p></body></html>
HTML
printf 'Dear Sam,\nThe lease for Flat 3 ends on 30 June 2027.\n' > letter-source.txt
textutil -convert rtf -output letter.rtf letter-source.txt
textutil -convert docx -output report.docx letter-source.txt
rm letter-source.txt

# A PDF with real text, then the same page as a picture, and as a scanned PDF.
printf 'INVOICE 4471\n\nBlue Bottle Coffee\nTotal due: 4.50\n' > invoice-source.txt
cupsfilter -o cpi=6 -o lpi=3 invoice-source.txt > invoice.pdf 2>/dev/null
rm invoice-source.txt
sips -s format png -s dpiWidth 200 -s dpiHeight 200 invoice.pdf --out receipt.png > /dev/null
sips -s format pdf receipt.png --out scan.pdf > /dev/null

# PowerPoint and Excel files, as small as they can be and still open.
python3 - <<'PY'
import zipfile
def write(name, files):
    with zipfile.ZipFile(name, "w", zipfile.ZIP_DEFLATED) as z:
        for path, text in files.items():
            z.writestr(path, text)
ct = '<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>'
write("slides.pptx", {
    "[Content_Types].xml": ct,
    "ppt/slides/slide2.xml": '<p:sld xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>Second slide: results</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>',
    "ppt/slides/slide1.xml": '<p:sld xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>Quarterly review</a:t></a:r></a:p><a:p><a:r><a:t>Sales up </a:t></a:r><a:r><a:t>12%</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>',
    "ppt/slides/slide10.xml": '<p:sld xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>Tenth slide</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>',
})
write("budget.xlsx", {
    "[Content_Types].xml": ct,
    "xl/workbook.xml": '<workbook xmlns="w" xmlns:r="r"><sheets><sheet name="Budget" sheetId="1" r:id="rId1"/><sheet name="Notes" sheetId="2" r:id="rId2"/></sheets></workbook>',
    "xl/_rels/workbook.xml.rels": '<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Target="worksheets/sheet2.xml"/></Relationships>',
    "xl/sharedStrings.xml": '<sst><si><t>Item</t></si><si><t>Cost</t></si><si><r><t>Rent </t></r><r><t>(March)</t></r></si></sst>',
    "xl/worksheets/sheet1.xml": '<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c></row><row r="2"><c r="A2" t="s"><v>2</v></c><c r="B2"><v>1200.5</v></c></row><row r="3"><c r="A3" t="inlineStr"><is><t>Total</t></is></c><c r="B3"><f>SUM(B2)</f><v>1200.5</v></c></row></sheetData></worksheet>',
    "xl/worksheets/sheet2.xml": '<worksheet><sheetData><row r="1"><c r="A1" t="str"><v>Paid on time</v></c><c r="B1" t="b"><v>1</v></c></row></sheetData></worksheet>',
})
PY

# Nothing readable in it.
printf '\000\001\002\003binary' > data.bin
