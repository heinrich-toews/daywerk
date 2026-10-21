// SPDX-License-Identifier: AGPL-3.0-only
// This file is part of Daywerk. See LICENSE for details.

//! Report rendering for Daywerk: XLSX for the office, PDF for printing.
//!
//! The renderers are pure: they receive a fully formatted [`Report`] and return
//! bytes. Timezones and localisation are resolved before the report is built.

use anyhow::Result;
use printpdf::*;
use rust_xlsxwriter::{Format, FormatAlign, Workbook};
use std::collections::BTreeMap;

pub const HOUR_MS: f64 = 3_600_000.0;

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub org_name: String,
    pub from: String,
    pub to: String,
    pub generated_at: String,
    pub workers: Vec<WorkerReport>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkerReport {
    pub name: String,
    pub email: String,
    pub org_name: String,
    pub days_count: i64,
    pub work_ms: i64,
    pub break_ms: i64,
    pub target_ms: i64,
    pub balance_ms: i64,
    pub total_balance_ms: i64,
    pub days: Vec<DayReport>,
}

#[derive(Debug, Clone, Default)]
pub struct DayReport {
    pub date: String,
    pub first_start: String,
    pub last_end: String,
    pub work_ms: i64,
    pub break_ms: i64,
    pub target_ms: i64,
    pub balance_ms: i64,
    pub segments: Vec<SegmentReport>,
}

#[derive(Debug, Clone, Default)]
pub struct SegmentReport {
    pub account: String,
    pub start: String,
    pub end: String,
    pub duration_ms: i64,
    pub note: String,
    pub material: String,
}

fn hours(ms: i64) -> f64 {
    (ms as f64 / HOUR_MS * 100.0).round() / 100.0
}

fn hhmm(ms: i64) -> String {
    let neg = ms < 0;
    let minutes = (ms.abs() as f64 / 60_000.0).round() as i64;
    let text = format!("{}:{:02}", minutes / 60, minutes % 60);
    if neg { format!("-{text}") } else { text }
}

// ---------------------------------------------------------------- XLSX

pub fn render_xlsx(report: &Report) -> Result<Vec<u8>> {
    let mut workbook = Workbook::new();

    let header = Format::new()
        .set_bold()
        .set_background_color(0x2A7647)
        .set_font_color(0xFFFFFF)
        .set_align(FormatAlign::Left);
    let bold = Format::new().set_bold();
    let number = Format::new().set_num_format("0.00");
    let number_bold = Format::new().set_num_format("0.00").set_bold();

    // ---- Übersicht -----------------------------------------------------
    let sheet = workbook.add_worksheet();
    sheet.set_name("Übersicht")?;

    let columns = [
        ("Mitarbeiter", 26),
        ("E-Mail", 26),
        ("Organisation", 20),
        ("Tage", 8),
        ("Arbeitszeit (h)", 16),
        ("Pause (h)", 12),
        ("Soll (h)", 12),
        ("Saldo Zeitraum (h)", 19),
        ("Saldo gesamt (h)", 18),
    ];
    for (index, (title, width)) in columns.iter().enumerate() {
        sheet.write_string_with_format(0, index as u16, *title, &header)?;
        sheet.set_column_width(index as u16, *width)?;
    }
    sheet.set_freeze_panes(1, 0)?;

    let mut work_sum = 0.0;
    let mut break_sum = 0.0;
    let mut target_sum = 0.0;
    let mut balance_sum = 0.0;

    for (index, worker) in report.workers.iter().enumerate() {
        let row = (index + 1) as u32;
        sheet.write_string(row, 0, &worker.name)?;
        sheet.write_string(row, 1, &worker.email)?;
        sheet.write_string(row, 2, &worker.org_name)?;
        sheet.write_number(row, 3, worker.days_count as f64)?;
        sheet.write_number_with_format(row, 4, hours(worker.work_ms), &number)?;
        sheet.write_number_with_format(row, 5, hours(worker.break_ms), &number)?;
        sheet.write_number_with_format(row, 6, hours(worker.target_ms), &number)?;
        sheet.write_number_with_format(row, 7, hours(worker.balance_ms), &number)?;
        sheet.write_number_with_format(row, 8, hours(worker.total_balance_ms), &number)?;

        work_sum += hours(worker.work_ms);
        break_sum += hours(worker.break_ms);
        target_sum += hours(worker.target_ms);
        balance_sum += hours(worker.balance_ms);
    }

    let total_row = (report.workers.len() + 1) as u32;
    sheet.write_string_with_format(total_row, 0, "Summe", &bold)?;
    sheet.write_number_with_format(total_row, 4, work_sum, &number_bold)?;
    sheet.write_number_with_format(total_row, 5, break_sum, &number_bold)?;
    sheet.write_number_with_format(total_row, 6, target_sum, &number_bold)?;
    sheet.write_number_with_format(total_row, 7, balance_sum, &number_bold)?;

    // ---- Tage ----------------------------------------------------------
    let sheet = workbook.add_worksheet();
    sheet.set_name("Tage")?;
    let day_columns = [
        ("Datum", 12),
        ("Mitarbeiter", 24),
        ("Beginn", 10),
        ("Ende", 10),
        ("Arbeitszeit (h)", 16),
        ("Pause (h)", 12),
        ("Soll (h)", 12),
        ("Saldo (h)", 12),
    ];
    for (index, (title, width)) in day_columns.iter().enumerate() {
        sheet.write_string_with_format(0, index as u16, *title, &header)?;
        sheet.set_column_width(index as u16, *width)?;
    }
    sheet.set_freeze_panes(1, 0)?;

    let mut row = 1u32;
    for worker in &report.workers {
        for day in &worker.days {
            sheet.write_string(row, 0, &day.date)?;
            sheet.write_string(row, 1, &worker.name)?;
            sheet.write_string(row, 2, &day.first_start)?;
            sheet.write_string(row, 3, &day.last_end)?;
            sheet.write_number_with_format(row, 4, hours(day.work_ms), &number)?;
            sheet.write_number_with_format(row, 5, hours(day.break_ms), &number)?;
            sheet.write_number_with_format(row, 6, hours(day.target_ms), &number)?;
            sheet.write_number_with_format(row, 7, hours(day.balance_ms), &number)?;
            row += 1;
        }
    }

    // ---- Segmente ------------------------------------------------------
    let sheet = workbook.add_worksheet();
    sheet.set_name("Segmente")?;
    let segment_columns = [
        ("Datum", 12),
        ("Mitarbeiter", 22),
        ("Zeitkonto", 20),
        ("Start", 8),
        ("Ende", 8),
        ("Dauer (h)", 11),
        ("Notiz", 40),
        ("Material", 40),
    ];
    for (index, (title, width)) in segment_columns.iter().enumerate() {
        sheet.write_string_with_format(0, index as u16, *title, &header)?;
        sheet.set_column_width(index as u16, *width)?;
    }
    sheet.set_freeze_panes(1, 0)?;

    let mut row = 1u32;
    for worker in &report.workers {
        for day in &worker.days {
            for segment in &day.segments {
                sheet.write_string(row, 0, &day.date)?;
                sheet.write_string(row, 1, &worker.name)?;
                sheet.write_string(row, 2, &segment.account)?;
                sheet.write_string(row, 3, &segment.start)?;
                sheet.write_string(row, 4, &segment.end)?;
                sheet.write_number_with_format(row, 5, hours(segment.duration_ms), &number)?;
                sheet.write_string(row, 6, &segment.note)?;
                sheet.write_string(row, 7, &segment.material)?;
                row += 1;
            }
        }
    }

    Ok(workbook.save_to_buffer()?)
}

// ---------------------------------------------------------------- PDF

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN: f32 = 20.0;
const LINE: f32 = 5.6;

struct Painter {
    doc: PdfDocumentReference,
    page: PdfPageIndex,
    layer: PdfLayerIndex,
    y: f32,
    font: IndirectFontRef,
    bold: IndirectFontRef,
}

impl Painter {
    fn new() -> Result<Self> {
        let (doc, page, layer) = PdfDocument::new("Daywerk", Mm(PAGE_W), Mm(PAGE_H), "Seite");
        let font = doc.add_builtin_font(BuiltinFont::Helvetica)?;
        let bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;
        Ok(Self {
            doc,
            page,
            layer,
            y: PAGE_H - MARGIN,
            font,
            bold,
        })
    }

    fn text(&mut self, x: f32, size: f32, value: &str, strong: bool) {
        if value.is_empty() {
            return;
        }
        let layer = self.doc.get_page(self.page).get_layer(self.layer);
        let font = if strong { &self.bold } else { &self.font };
        layer.use_text(value, size, Mm(x), Mm(self.y), font);
    }

    fn row(&mut self, cells: &[(f32, String)], size: f32, strong: bool) {
        for (x, value) in cells {
            self.text(*x, size, value, strong);
        }
        self.y -= LINE;
    }

    fn gap(&mut self, mm: f32) {
        self.y -= mm;
    }

    fn new_page(&mut self) -> Result<()> {
        let (page, layer) = self.doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Seite");
        self.page = page;
        self.layer = layer;
        self.y = PAGE_H - MARGIN;
        Ok(())
    }

    fn ensure(&mut self, needed: f32) -> Result<()> {
        if self.y - needed < MARGIN {
            self.new_page()?;
        }
        Ok(())
    }

    fn save(self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        self.doc
            .save(&mut std::io::BufWriter::new(&mut buffer))
            .map_err(|e| anyhow::anyhow!("pdf: {e}"))?;
        Ok(buffer)
    }
}

fn truncate(value: &str, max: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max {
        value.to_string()
    } else {
        format!(
            "{}…",
            chars[..max.saturating_sub(1)].iter().collect::<String>()
        )
    }
}

pub fn render_pdf(report: &Report) -> Result<Vec<u8>> {
    let mut painter = Painter::new()?;

    painter.text(MARGIN, 20.0, &truncate(&report.org_name, 60), true);
    painter.gap(9.0);
    painter.text(
        MARGIN,
        12.0,
        &format!("Zeiten {} – {}", report.from, report.to),
        false,
    );
    painter.gap(6.0);
    painter.text(
        MARGIN,
        8.0,
        &format!("Erstellt am {}", report.generated_at),
        false,
    );
    painter.gap(10.0);

    // ---- summary -------------------------------------------------------
    painter.row(
        &[
            (MARGIN, "Mitarbeiter".into()),
            (95.0, "Tage".into()),
            (112.0, "Arbeit".into()),
            (134.0, "Pause".into()),
            (152.0, "Saldo".into()),
        ],
        8.0,
        true,
    );
    for worker in &report.workers {
        painter.ensure(LINE * 2.0)?;
        painter.row(
            &[
                (MARGIN, truncate(&worker.name, 42)),
                (95.0, worker.days_count.to_string()),
                (112.0, hhmm(worker.work_ms)),
                (134.0, hhmm(worker.break_ms)),
                (152.0, hhmm(worker.balance_ms)),
            ],
            8.0,
            false,
        );
    }

    painter.gap(6.0);

    // ---- per worker detail --------------------------------------------
    for worker in &report.workers {
        painter.ensure(LINE * 4.0)?;
        painter.gap(4.0);
        painter.text(
            MARGIN,
            11.0,
            &truncate(&format!("{} ({})", worker.name, worker.org_name), 70),
            true,
        );
        painter.gap(6.0);
        painter.row(
            &[
                (MARGIN, "Datum".into()),
                (50.0, "Beginn".into()),
                (68.0, "Ende".into()),
                (86.0, "Arbeit".into()),
                (106.0, "Pause".into()),
                (124.0, "Saldo".into()),
            ],
            7.5,
            true,
        );
        for day in &worker.days {
            painter.ensure(LINE * 2.0)?;
            painter.row(
                &[
                    (MARGIN, day.date.clone()),
                    (50.0, day.first_start.clone()),
                    (68.0, day.last_end.clone()),
                    (86.0, hhmm(day.work_ms)),
                    (106.0, hhmm(day.break_ms)),
                    (124.0, hhmm(day.balance_ms)),
                ],
                7.5,
                false,
            );
        }
        painter.gap(4.0);
    }

    painter.save()
}

/// Group segments by day for a compact overview (unused by the renderers, kept
/// for callers that want to inspect the model).
pub fn group_by_day<'a>(
    segments: impl Iterator<Item = (&'a str, &'a SegmentReport)>,
) -> BTreeMap<String, Vec<&'a SegmentReport>> {
    let mut map: BTreeMap<String, Vec<&SegmentReport>> = BTreeMap::new();
    for (date, segment) in segments {
        map.entry(date.to_string()).or_default().push(segment);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Report {
        Report {
            org_name: "Test GmbH".into(),
            from: "01.10.2026".into(),
            to: "31.10.2026".into(),
            generated_at: "21.10.2026 18:00".into(),
            workers: vec![WorkerReport {
                name: "Anna Müller".into(),
                email: "anna@example.org".into(),
                org_name: "Test GmbH".into(),
                days_count: 1,
                work_ms: 8 * 3_600_000,
                break_ms: 30 * 60_000,
                target_ms: 8 * 3_600_000,
                balance_ms: 0,
                total_balance_ms: 90 * 60_000,
                days: vec![DayReport {
                    date: "21.10.2026".into(),
                    first_start: "08:00".into(),
                    last_end: "16:30".into(),
                    work_ms: 8 * 3_600_000,
                    break_ms: 30 * 60_000,
                    target_ms: 8 * 3_600_000,
                    balance_ms: 0,
                    segments: vec![SegmentReport {
                        account: "ALLGEMEIN".into(),
                        start: "08:00".into(),
                        end: "16:30".into(),
                        duration_ms: 8 * 3_600_000,
                        note: "Anschluss".into(),
                        material: "2x Rohr".into(),
                    }],
                }],
            }],
        }
    }

    #[test]
    fn xlsx_has_a_zip_header() {
        let bytes = render_xlsx(&sample()).unwrap();
        assert_eq!(&bytes[..4], b"PK\x03\x04");
        assert!(bytes.len() > 2_000);
    }

    #[test]
    fn pdf_has_a_pdf_header_and_eof() {
        let bytes = render_pdf(&sample()).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.ends_with(b"%%EOF\n") || bytes.ends_with(b"%%EOF"));
        assert!(bytes.len() > 1_000);
    }

    #[test]
    fn empty_report_still_renders() {
        let report = Report {
            org_name: "Leer".into(),
            from: "01.10.2026".into(),
            to: "31.10.2026".into(),
            generated_at: "21.10.2026 18:00".into(),
            workers: vec![],
        };
        assert!(render_xlsx(&report).unwrap().len() > 1_000);
        assert!(render_pdf(&report).unwrap().starts_with(b"%PDF-"));
    }
}
