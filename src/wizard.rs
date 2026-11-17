//! Interactive front end: collect ordinary CLI arguments, never execute a shell.

use std::io::{self, BufRead, Write};
use std::path::Path;

use karaoke_cli::catalog::{self, ModelInfo};
use karaoke_cli::download::WHISPER_SIZES;
use karaoke_cli::pipeline::{validate_output_options, ChunkConfig, OutputFormat};

enum Failure {
    Cancelled,
    Io(io::Error),
    Other(String),
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::Interrupted {
            Self::Cancelled
        } else {
            Self::Io(error)
        }
    }
}

type Result<T> = std::result::Result<T, Failure>;

pub fn run(
    reader: impl BufRead,
    writer: impl Write,
    banner: &str,
    executable: &str,
) -> std::result::Result<Option<Vec<String>>, String> {
    let mut prompts = Prompts { reader, writer };
    match prompts.collect(banner, executable) {
        Ok(args) => Ok(Some(args)),
        Err(Failure::Cancelled) => {
            writeln!(prompts.writer, "\nОтменено.").map_err(|error| error.to_string())?;
            Ok(None)
        }
        Err(Failure::Io(error)) => Err(format!("wizard: {error}")),
        Err(Failure::Other(error)) => Err(error),
    }
}

struct Prompts<R, W> {
    reader: R,
    writer: W,
}

impl<R: BufRead, W: Write> Prompts<R, W> {
    fn line(&mut self, question: &str, default: &str) -> Result<String> {
        if default.is_empty() {
            write!(self.writer, "{question}: ")?;
        } else {
            write!(self.writer, "{question} [{default}]: ")?;
        }
        self.writer.flush()?;
        let mut line = String::new();
        if self.reader.read_line(&mut line)? == 0 {
            return Err(Failure::Cancelled);
        }
        let value = line.trim();
        if value.eq_ignore_ascii_case(":q") {
            return Err(Failure::Cancelled);
        }
        Ok(if value.is_empty() { default } else { value }.to_owned())
    }

    fn choice(&mut self, title: &str, labels: &[&str], default: usize) -> Result<usize> {
        writeln!(self.writer, "\n{title}")?;
        for (index, label) in labels.iter().enumerate() {
            writeln!(self.writer, "  {}. {label}", index + 1)?;
        }
        loop {
            let answer = self.line("Выберите номер", &(default + 1).to_string())?;
            if let Ok(number) = answer.parse::<usize>() {
                if (1..=labels.len()).contains(&number) {
                    return Ok(number - 1);
                }
            }
            writeln!(self.writer, "Введите номер от 1 до {}.", labels.len())?;
        }
    }

    fn yes(&mut self, question: &str, default: bool) -> Result<bool> {
        loop {
            match self
                .line(question, if default { "да" } else { "нет" })?
                .to_lowercase()
                .as_str()
            {
                "y" | "yes" | "д" | "да" => return Ok(true),
                "n" | "no" | "н" | "нет" => return Ok(false),
                _ => writeln!(self.writer, "Введите да/нет (y/n).")?,
            }
        }
    }

    fn path(&mut self, question: &str, default: &str, existing: bool) -> Result<String> {
        loop {
            let answer = self.line(question, default)?;
            // Accept pasted/dragged console paths, including quotes and spaces.
            let path = unquote(&answer);
            if !path.is_empty()
                && if existing {
                    Path::new(path).exists()
                } else {
                    !Path::new(path).exists() || Path::new(path).is_dir()
                }
            {
                return Ok(path.to_owned());
            }
            writeln!(
                self.writer,
                "Укажите {}путь.",
                if existing {
                    "существующий "
                } else {
                    ""
                }
            )?;
        }
    }

    fn whisper(&mut self, args: &mut Vec<String>, directory: &str) -> Result<()> {
        args.push("--transcribe".into());
        let default = WHISPER_SIZES
            .iter()
            .position(|size| *size == "small")
            .unwrap();
        let size = WHISPER_SIZES[self.choice(
            "Модель Whisper (больше = больше памяти и загрузка)",
            WHISPER_SIZES,
            default,
        )?];
        add(args, "--whisper_size", size);
        let whisper_dir = Path::new(directory).join(format!("whisper-{size}"));
        let dir = self.path("Папка Whisper", &whisper_dir.to_string_lossy(), false)?;
        add(args, "--whisper_model_dir", &dir);
        Ok(())
    }

    fn model(
        &mut self,
        args: &mut Vec<String>,
        models: &[ModelInfo],
        prepare: bool,
    ) -> Result<bool> {
        let mut labels: Vec<_> = models.iter().map(|model| model.name.as_str()).collect();
        if !prepare {
            labels.push("Своя локальная модель (.safetensors + YAML)");
        }
        let default = models
            .iter()
            .position(|model| model.filename == "mbr_instfv6n_gabox.safetensors")
            .unwrap_or(0);
        let selected = self.choice("Модель разделения", &labels, default)?;
        if let Some(model) = models.get(selected) {
            let stem = Path::new(&model.filename)
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap();
            add(args, "--model", stem);
            Ok(true)
        } else {
            loop {
                let path = self.path("Файл весов .safetensors", "", true)?;
                if Path::new(&path).is_file()
                    && Path::new(&path)
                        .extension()
                        .and_then(|value| value.to_str())
                        == Some("safetensors")
                {
                    let absolute = std::fs::canonicalize(&path)?;
                    add(args, "--model", &absolute.to_string_lossy());
                    let default_config = Path::new(&path).with_extension("yaml");
                    loop {
                        let config =
                            self.path("Конфиг YAML", &default_config.to_string_lossy(), true)?;
                        if Path::new(&config).is_file() {
                            let absolute = std::fs::canonicalize(&config)?;
                            add(args, "--config", &absolute.to_string_lossy());
                            return Ok(false);
                        }
                        writeln!(self.writer, "Нужен файл конфига, а не папка.")?;
                    }
                }
                writeln!(self.writer, "Нужен существующий файл .safetensors.")?;
            }
        }
    }

    fn collect(&mut self, banner: &str, executable: &str) -> Result<Vec<String>> {
        writeln!(
            self.writer,
            "{banner}\nПошаговая настройка. Enter — значение в скобках; :q — отмена."
        )?;
        let action = self.choice(
            "Что сделать?",
            &[
                "Обработать аудиофайл или папку",
                "Скачать / подготовить модели",
                "Показать сведения о системе",
                "Показать каталог моделей",
                "Выйти",
            ],
            0,
        )?;
        match action {
            2 => return Ok(vec!["--env_info".into()]),
            3 => return Ok(vec!["--list_models".into()]),
            4 => return Err(Failure::Cancelled),
            _ => {}
        }
        let prepare = action == 1;
        let mut args = Vec::new();
        let mut inputs = Vec::new();
        if !prepare {
            let input = self.path("Аудиофайл или папка с аудио", "", true)?;
            inputs.push(input);
            loop {
                let input = self.line("Ещё файл/папка (Enter — дальше)", "")?;
                if input.is_empty() {
                    break;
                }
                let path = unquote(&input);
                if Path::new(path).exists() {
                    inputs.push(path.to_owned());
                } else {
                    writeln!(self.writer, "Путь не существует.")?;
                }
            }
        }
        let models = catalog::builtin_models().map_err(Failure::Other)?;
        let builtin = self.model(&mut args, &models, prepare)?;
        let directory = self.path("Папка моделей", "models", false)?;
        if builtin {
            add(&mut args, "--model_file_dir", &directory);
        }

        let transcribe = self.yes(
            if prepare {
                "Также подготовить Whisper?"
            } else {
                "Распознать текст песни (Whisper)?"
            },
            false,
        )?;
        if transcribe {
            self.whisper(&mut args, &directory)?;
            if !prepare {
                loop {
                    let language = self
                        .line("Язык текста (ru, en, auto...)", "ru")?
                        .to_lowercase();
                    if language == "auto"
                        || ((2..=3).contains(&language.len())
                            && language.bytes().all(|byte| byte.is_ascii_alphabetic()))
                    {
                        add(&mut args, "--language", &language);
                        break;
                    }
                    writeln!(self.writer, "Введите код языка (например, ru) или auto.")?;
                }
            }
        }
        if prepare || self.yes("Разрешить скачивание недостающих моделей?", true)?
        {
            args.push("--download_models".into());
        }
        if !prepare {
            let mut devices = vec!["CPU"];
            let mut device_values = vec!["cpu"];
            if cfg!(feature = "cuda") {
                devices.push("CUDA (NVIDIA)");
                device_values.push("cuda");
            }
            if cfg!(feature = "metal") {
                devices.push("Metal (Apple)");
                device_values.push("metal");
            }
            let device = self.choice("Устройство (доступны варианты этой сборки)", &devices, 0)?;
            add(&mut args, "--device", device_values[device]);
            let out = self.path("Папка результатов", "out", false)?;
            add(&mut args, "--output_dir", &out);
            let format = self.choice("Формат аудио", &["WAV", "FLAC", "MP3"], 0)?;
            add(&mut args, "--output_format", ["wav", "flac", "mp3"][format]);
            if format == 2 {
                loop {
                    let bitrate = self.line("Битрейт MP3 (кбит/с)", "192")?;
                    if let Ok(bitrate) = bitrate.parse::<u16>() {
                        if validate_output_options(OutputFormat::Mp3, Some(bitrate), None).is_ok() {
                            add(&mut args, "--output_bitrate", &bitrate.to_string());
                            break;
                        }
                    }
                    writeln!(self.writer, "Недопустимый битрейт MP3.")?;
                }
            }
            let stem = self.choice(
                "Какие дорожки сохранить?",
                &["Все", "Только instrumental", "Только vocals"],
                0,
            )?;
            if stem > 0 {
                add(
                    &mut args,
                    "--single_stem",
                    ["", "instrumental", "vocals"][stem],
                );
            }
            writeln!(
                self.writer,
                "\nОбработка по частям (чанкам): песня делится на небольшие отрезки,"
            )?;
            writeln!(
                self.writer,
                "которые обрабатываются по очереди и плавно соединяются обратно."
            )?;
            writeln!(
                self.writer,
                "Меньше отрезок — меньше расход памяти, но больше накладных расходов"
            )?;
            writeln!(
                self.writer,
                "на обработку; на стыках возможно небольшое изменение качества."
            )?;
            writeln!(
                self.writer,
                "20 секунд — начальная настройка. Если не хватает памяти, уменьшите её."
            )?;
            writeln!(
                self.writer,
                "0 — обработать всю песню целиком: может потребоваться значительно больше памяти."
            )?;
            loop {
                let seconds = self.line("Длина одного отрезка, секунд (0 — вся песня)", "20")?;
                if let Ok(seconds) = seconds.parse::<f64>() {
                    if seconds == 0.0 {
                        break;
                    }
                    if ChunkConfig::new(seconds, None, 0.9, 0.0).validate().is_ok() {
                        add(&mut args, "--chunk_seconds", &seconds.to_string());
                        break;
                    }
                }
                writeln!(
                    self.writer,
                    "Введите допустимое положительное число секунд или 0."
                )?;
            }
            if self.yes("Перезаписывать существующие результаты?", false)?
            {
                args.push("--overwrite".into());
            }
        }
        // Keep positional paths that begin with '-' from becoming CLI options.
        if !inputs.is_empty() {
            args.push("--".into());
            args.extend(inputs);
        }
        writeln!(
            self.writer,
            "\nНастройки (команда для {}):\n{}",
            if cfg!(windows) { "PowerShell" } else { "Bash" },
            command(executable, &args)
        )?;
        writeln!(
            self.writer,
            "Скачивание и обработка начнутся только после подтверждения."
        )?;
        if !self.yes("Начать?", false)? {
            return Err(Failure::Cancelled);
        }
        Ok(args)
    }
}

fn add(args: &mut Vec<String>, flag: &str, value: &str) {
    args.extend([flag.to_owned(), value.to_owned()]);
}

fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(value) = value
            .strip_prefix(quote)
            .and_then(|value| value.strip_suffix(quote))
        {
            return value;
        }
    }
    value
}

fn command(executable: &str, args: &[String]) -> String {
    command_for(
        if cfg!(windows) {
            Shell::PowerShell
        } else {
            Shell::Bash
        },
        executable,
        args,
    )
}

#[derive(Clone, Copy)]
enum Shell {
    PowerShell,
    Bash,
}

fn command_for(shell: Shell, executable: &str, args: &[String]) -> String {
    let quote = |value: &str, force: bool| {
        if !force
            && !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_./:-".contains(&byte))
        {
            return value.to_owned();
        }
        match shell {
            Shell::PowerShell => {
                // Double quotes leave apostrophes readable, while escaping
                // PowerShell interpolation, escape characters and quotes.
                format!(
                    "\"{}\"",
                    value
                        .replace('`', "``")
                        .replace('$', "`$")
                        .replace('"', "`\"")
                )
            }
            Shell::Bash => format!("'{}'", value.replace('\'', "'\\''")),
        }
    };
    let (prefix, continuation) = match shell {
        Shell::PowerShell => ("& ", " `\n  "),
        Shell::Bash => ("", " \\\n  "),
    };
    let mut lines = vec![format!("{prefix}{}", quote(executable, true))];
    let mut positional = false;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--" && !positional {
            positional = true;
            lines.push("--".into());
            index += 1;
        } else if !positional
            && arg.starts_with("--")
            && index + 1 < args.len()
            && !args[index + 1].starts_with("--")
        {
            lines.push(format!(
                "{} {}",
                quote(arg, false),
                quote(&args[index + 1], false)
            ));
            index += 2;
        } else {
            lines.push(quote(arg, false));
            index += 1;
        }
    }
    lines.join(continuation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scripted(script: &str) -> (Option<Vec<String>>, String) {
        let mut output = Vec::new();
        let args = run(
            script.as_bytes(),
            &mut output,
            "test version",
            "karaoke-cli",
        )
        .unwrap();
        (args, String::from_utf8(output).unwrap())
    }

    #[test]
    fn exit_eof_and_cancel_never_start_work() {
        for script in ["", "5\n", ":q\n", "2\n\n\nn\nn\n", "2\n"] {
            let (args, output) = scripted(script);
            assert!(args.is_none(), "{script:?}");
            assert!(output.contains("Отменено."));
        }
    }

    #[test]
    fn read_only_menu_actions_reuse_cli_flags() {
        assert_eq!(scripted("3\n").0.unwrap(), ["--env_info"]);
        assert_eq!(scripted("4\n").0.unwrap(), ["--list_models"]);
    }

    #[test]
    fn prepare_requires_confirmation_and_uses_default_catalog_model() {
        let (args, output) = scripted("2\n\n\nn\ny\n");
        let parsed = crate::parse_args(&args.unwrap()).unwrap();
        assert!(parsed.download_models);
        assert!(parsed.inputs.is_empty());
        assert_eq!(parsed.model.as_deref(), Some("mbr_instfv6n_gabox"));
        assert_eq!(parsed.model_file_dir.as_deref(), Some("models"));
        assert!(!parsed.transcribe);
        assert!(output.contains("Начать?"));
        assert!(output.contains("--download_models"));
    }

    #[test]
    fn prepare_can_include_whisper_large_v3() {
        let (args, _) = scripted("2\n\n\nда\n10\n\nда\n");
        let parsed = crate::parse_args(&args.unwrap()).unwrap();
        assert!(parsed.transcribe);
        assert_eq!(parsed.whisper_size, "large-v3");
        assert_eq!(
            parsed.whisper_model_dir.as_deref(),
            Some(
                Path::new("models")
                    .join("whisper-large-v3")
                    .to_str()
                    .unwrap()
            )
        );
    }

    #[test]
    fn invalid_menu_answers_are_reprompted() {
        let (args, output) = scripted("nonsense\n0\n100\n3\n");
        assert_eq!(args.unwrap(), ["--env_info"]);
        assert_eq!(output.matches("Введите номер от").count(), 3);
    }

    #[test]
    fn processing_keeps_quoted_unicode_paths_and_validates_chunk_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("Кипелов Ya Zdes'.mp3");
        std::fs::write(&input, b"fixture").unwrap();
        let script = [
            "",
            &format!("\"{}\"", input.display()),
            "",
            "",
            "",
            "n",
            "y",
            "1",
            "",
            "1",
            "2",
            "NaN",
            "-1",
            "20",
            "n",
            "y",
            "",
        ]
        .join("\n");
        let (args, output) = scripted(&script);
        let parsed = crate::parse_args(&args.unwrap()).unwrap();
        assert_eq!(parsed.inputs, [input]);
        assert!(parsed.download_models);
        assert_eq!(parsed.single_stem.as_deref(), Some("instrumental"));
        assert_eq!(parsed.chunk_seconds, Some(20.0));
        assert!(!parsed.overwrite);
        assert!(output.contains("Введите допустимое"));
        assert!(output.contains("песня делится на небольшие отрезки"));
        assert!(output.contains("0 — обработать всю песню целиком"));
        assert!(!dir.path().join("models").exists());
        assert!(!dir.path().join("out").exists());
    }

    #[test]
    fn command_output_uses_shell_specific_quotes_and_continuations() {
        let args = [
            "--model",
            "mbr_instfv6n_gabox",
            "--overwrite",
            "--",
            "F:\\Кипелов\\Ya Zdes'.mp3",
        ]
        .map(str::to_owned);
        let pwsh = command_for(Shell::PowerShell, "F:\\karaoke-cli.exe", &args);
        assert_eq!(pwsh, "& \"F:\\karaoke-cli.exe\" `\n  --model mbr_instfv6n_gabox `\n  --overwrite `\n  -- `\n  \"F:\\Кипелов\\Ya Zdes'.mp3\"");
        let bash = command_for(Shell::Bash, "/tools/karaoke-cli", &args);
        assert_eq!(bash, "'/tools/karaoke-cli' \\\n  --model mbr_instfv6n_gabox \\\n  --overwrite \\\n  -- \\\n  'F:\\Кипелов\\Ya Zdes'\\''.mp3'");
        assert!(!pwsh.lines().last().unwrap().ends_with('`'));
        assert!(!bash.lines().last().unwrap().ends_with('\\'));
    }

    #[test]
    #[cfg(windows)]
    fn generated_powershell_command_roundtrips_special_characters() {
        let args = [
            "--model",
            "mbr_instfv6n_gabox",
            "--config",
            "F:\\Кипелов\\Ya Zdes'.yaml",
            "--output_dir",
            "F:\\spaces $HOME ` \"quote\" ; (stuff)\\",
            "",
            "#comment",
            "$(throw 'injection')",
        ]
        .map(str::to_owned);
        let script = format!(
            "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)\nfunction Capture-Arguments {{ ConvertTo-Json -Compress -InputObject @($args) }}\n{}",
            command_for(Shell::PowerShell, "Capture-Arguments", &args)
        );
        let output = std::process::Command::new("pwsh")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual, args);
    }

    #[test]
    #[cfg(unix)]
    fn generated_bash_command_roundtrips_special_characters() {
        let args = [
            "--model",
            "mbr_instfv6n_gabox",
            "--",
            "/Кипелов/Ya Zdes'.mp3",
            "/spaces $HOME ` \"quote\" ; (stuff)/",
            "",
            "#comment",
            "$(exit 42)",
        ]
        .map(str::to_owned);
        let script = format!(
            "capture() {{ printf '%s\\0' \"$@\"; }}\n{}",
            command_for(Shell::Bash, "capture", &args)
        );
        let output = std::process::Command::new("bash")
            .args(["-c", &script])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual: Vec<_> = output
            .stdout
            .split(|byte| *byte == 0)
            .take(args.len())
            .map(|bytes| String::from_utf8(bytes.to_vec()).unwrap())
            .collect();
        assert_eq!(actual, args);
    }
}
