//! Parser argumen CLI sederhana (tanpa dependency tambahan).
//!
//! Perintah dikirim ke instans yang berjalan melalui socket IPC, sehingga dapat
//! dipakai langsung dari `binds` Niri tanpa membuat shortcut global sendiri.

use crate::error::{AppError, Result};
use crate::ipc::IpcCommand;

#[derive(Debug, PartialEq, Eq)]
pub enum CliAction {
    /// Jalankan aplikasi (widget). `smoke` = jalankan lalu keluar (uji start).
    Run { smoke: bool },
    /// Kirim perintah ke instans yang berjalan.
    Ipc(IpcCommand),
    Help,
    Version,
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<CliAction> {
    let args: Vec<String> = args.into_iter().collect();
    let mut smoke = false;
    let mut command: Option<String> = None;

    for arg in &args {
        match arg.as_str() {
            "--help" | "-h" => return Ok(CliAction::Help),
            "--version" | "-V" => return Ok(CliAction::Version),
            "--smoke-test" => smoke = true,
            other if other.starts_with('-') => {
                return Err(AppError::Config(format!("argumen tidak dikenal: {other}")));
            }
            other => {
                if command.is_some() {
                    return Err(AppError::Config(
                        "hanya satu subperintah yang diperbolehkan".into(),
                    ));
                }
                command = Some(other.to_string());
            }
        }
    }

    if let Some(cmd) = command {
        let parsed = IpcCommand::parse(&cmd)
            .ok_or_else(|| AppError::Config(format!("subperintah tidak dikenal: {cmd}")))?;
        return Ok(CliAction::Ipc(parsed));
    }
    Ok(CliAction::Run { smoke })
}

pub fn help_text() -> String {
    format!(
        "niri-ai-copilot {VERSION} — asisten interview AI untuk Niri (Wayland)\n\n\
         PEMAKAIAN:\n  \
         niri-ai-copilot [opsi] [subperintah]\n\n\
         OPSI:\n  \
         -h, --help       Tampilkan bantuan\n  \
         -V, --version    Tampilkan versi\n      \
         --smoke-test     Jalankan lalu keluar segera (uji start GUI)\n\n\
         SUBPERINTAH (dikirim ke instans yang berjalan via socket IPC):\n  \
         toggle           Tampilkan/sembunyikan widget\n  \
         show / hide      Tampilkan / sembunyikan widget\n  \
         start            Mulai sesi (butuh konfirmasi izin di UI)\n  \
         pause / resume   Jeda / lanjutkan perekaman\n  \
         stop             Akhiri sesi\n  \
         expand/collapse  Perluas / kecilkan widget\n  \
         alt-answer       Minta alternatif jawaban terakhir\n  \
         status           Cetak status aplikasi\n  \
         quit             Keluar dari aplikasi\n\n\
         Contoh integrasi Niri (tambahkan manual ke ~/.config/niri/config.kdl):\n  \
         binds {{\n    \
         Mod+Shift+C {{ spawn-sh \"niri-ai-copilot toggle\"; }}\n    \
         Mod+Shift+X {{ spawn-sh \"niri-ai-copilot start\"; }}\n  \
         }}\n\n\
         Catatan: perekaman hanya dimulai setelah Anda mengonfirmasi izin di UI."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Result<CliAction> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_to_run() {
        assert_eq!(parse_strs(&[]).unwrap(), CliAction::Run { smoke: false });
    }

    #[test]
    fn smoke_flag() {
        assert_eq!(
            parse_strs(&["--smoke-test"]).unwrap(),
            CliAction::Run { smoke: true }
        );
    }

    #[test]
    fn subcommands_map_to_ipc() {
        assert_eq!(parse_strs(&["toggle"]).unwrap(), CliAction::Ipc(IpcCommand::Toggle));
        assert_eq!(parse_strs(&["start"]).unwrap(), CliAction::Ipc(IpcCommand::Start));
        assert_eq!(parse_strs(&["alt-answer"]).unwrap(), CliAction::Ipc(IpcCommand::AltAnswer));
    }

    #[test]
    fn help_and_version() {
        assert_eq!(parse_strs(&["--help"]).unwrap(), CliAction::Help);
        assert_eq!(parse_strs(&["-V"]).unwrap(), CliAction::Version);
    }

    #[test]
    fn unknown_arguments_error() {
        assert!(parse_strs(&["--bogus"]).is_err());
        assert!(parse_strs(&["frobnicate"]).is_err());
    }
}
