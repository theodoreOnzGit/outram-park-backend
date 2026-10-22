//! The download progress bars at the bottom of the window (GitHub issue
//! #1016): one row per corpus download running off the GUI thread, from the
//! [`DownloadProgress`] the job writes. The window only reads a snapshot each
//! frame, so it stays responsive however long Git takes.

use crate::corpus_repos::DownloadProgress;

/// Draw one row per download: a bar (a spinner while Git gives no
/// percentage), the label ([`crate::corpus_repos::CloneProgress::label`]:
/// what, which repository, the phase, MiB and MiB/s) and, for a plain clone,
/// a Cancel button.
///
/// GUI drawing code, not reached by a test (workspace rule exemption); the
/// text, the fill and the cancel it acts on are tested in
/// `corpus_repos::progress`.
pub(super) fn ui(ui: &mut egui::Ui, downloads: &[DownloadProgress]) {
    for download in downloads {
        let s = download.snapshot();
        ui.horizontal_wrapped(|ui| {
            match s.fraction() {
                Some(f) => {
                    ui.add(egui::ProgressBar::new(f).desired_width(160.0));
                }
                None => {
                    ui.spinner();
                }
            }
            ui.label(s.label());
            if s.cancel_requested {
                ui.weak("cancelling…");
            } else if s.cancellable
                && ui
                    .button("Cancel")
                    .on_hover_text("Stop the download and put the folder back as it was")
                    .clicked()
            {
                download.request_cancel();
            }
        });
    }
}
