# Cycle status: open-issues / cycle 1

## Current state

- task: `open-issues` · form: `cycle` · specialized skill: `pdca-coder`
- mode: `autonomous`
- cycle: `N=1` · revision: `r=1` · attempt: `n=2/3`
- scope: implementation of all requirements in open repository tickets `#1`–`#7`, dependency update to `candle-roformer` `1.0`, documentation and authorized local commits · status path: `.pdca/status/open-issues-1.md` · evidence root: `.pdca/open-issues/rv1/`
- goal: deliver the updated package and CLI with all seven open tickets implemented and confirmed, documentation updated, and changes committed locally — the §PLAN record and §Original task are authoritative
- phase: `DO` · record: `open` · outcome: `candidate pending independent CHECK`
- source: user request · context refs: `/home/alex/sources/karaoke-cli`; baseline `main` at `a1ea0a4cea0d7d19142531d91e91469e731af2e8`
- criteria: `AC-1`–`AC-9` — exact text in §PLAN record `r1` (source: this file, `## PLAN record` → `### r1`)

## Original task

вышла новая версия 1.0, там закрыты все тикеты, которые нужны были для реализации открытых тикетов проекта
запускай pdca по открытым тикетам автономно автокомит

## PLAN record

### r1

````text
# План цикла PDCA (PLAN, `r = 1`)

## 1. Форма, специализированный скилл и входной объём

- **Форма (Form):** `cycle` (обычный цикл).
- **Специализированный скилл / оверлей (Specialized skill / overlay):** `pdca-coder` (разработка ПО на Rust).
- **Обоснование формы в одну строку:** Метод реализации (обновление зависимости `candle-roformer` до `1.0` и реализация связанных возможностей API/CLI в единой кодовой базе `src/lib.rs`, `src/main.rs`, `src/pipeline.rs`, `tests/flow.rs`) полностью известен, а объём ограничен ровно 7 открытыми тикетами репозитория (`#1`–`#7`), чьи изменения последовательно зависят от одних и тех же структур конфигурации, именования стемов и конвейера вывода.
- **Входной объём (Entry scope):** Полная реализация требований открытых тикетов `#1`–`#7` проекта `karaoke-cli` поверх `candle-roformer` `1.0`, обновление пользовательской документации и фиксация результата локальными git-коммитами в `/home/alex/sources/karaoke-cli`.
- **Привязка к снимку (Re-anchor snapshot) и расположение доказательств (Evidence location):** Текущее рабочее дерево репозитория `/home/alex/sources/karaoke-cli` (ветка `main`, включая незакоммиченные правки перехода на опубликованный `candle-roformer`). Корень привязки доказательств — основной checkout `/home/alex/sources/karaoke-cli`.

---

## 2. Цель и критерии приёмки

### Исходная задача пользователя (дословно)

> вышла новая версия 1.0, там закрыты все тикеты, которые нужны были для реализации открытых тикетов проекта
> запускай pdca по открытым тикетам автономно автокомит

### Запрашиваемый и поставляемый результат

- **Запрашиваемый результат (Requested result):** Обновлённый до `candle-roformer` `1.0` пакет и CLI `karaoke-cli`, в котором реализованы и подтверждены все 7 открытых тикетов репозитория (`#1`–`#7`), актуализирована документация и изменения зафиксированы локальными git-коммитами в `/home/alex/sources/karaoke-cli`.
- **Поставляемый результат (Delivered result):** Обновлённый до `candle-roformer` `1.0` пакет и CLI `karaoke-cli`, в котором реализованы и подтверждены все 7 открытых тикетов репозитория (`#1`–`#7`), актуализирована документация и изменения зафиксированы локальными git-коммитами в `/home/alex/sources/karaoke-cli` (тот же самый объект).

### Критерии приёмки (производные от неизменённой задачи и тел открытых тикетов `#1`–`#7`)

- **`AC-1` (Переход на `candle-roformer` 1.0):** Проект `karaoke-cli` использует опубликованную версию `candle-roformer` `1.0` (`1.0.x` с crates.io) в `Cargo.toml`/`Cargo.lock` без локальных path-зависимостей и компилируется без ошибок.
- **`AC-2` (Тикет #1 — входные директории и агрегация пакетных ошибок):**
  - CLI принимает на вход пути к директориям наряду с файлами, выполняет нерекурсивное обнаружение поддерживаемых аудиофайлов в детерминированном (стабильном) порядке;
  - предварительная проверка (preflight) с учётом `--overwrite` выполняется пофайлово: для отклонённого файла запись на диск не производится;
  - действует семантика continue-on-error: сбой на одном входе не прерывает обработку остальных входов пакета (при наличии одного валидного и одного повреждённого файла результат для валидного файла успешно создаётся);
  - ошибки собираются по каждому входу, в выводе указываются причины по каждому неуспешному файлу и итоговая сводка со списком всех неуспешных входов, а код возврата процесса ненулевой, если хотя бы один вход завершился ошибкой.
- **`AC-3` (Тикет #2 — `--custom_output_names`):**
  - Флаг `--custom_output_names '<json>'` принимает JSON-отображение `имя_стема -> базовое_имя_файла` (как для двухстемового режима, так и для именованных стемов многостемового API);
  - поддерживаются частичные отображения (не указанные в JSON стемы сохраняют стандартное именование `<input>_<Stem>_<model>.<ext>`);
  - сопутствующие текстовые и JSON-файлы транскрипции (`.txt`/`.json` sidecars) используют то же пользовательское базовое имя, что и вокальный стем;
  - неизвестные имена стемов, некорректный JSON, абсолютные пути и обход директорий (`..`) детерминированно отвергаются с понятной ошибкой без создания файлов.
- **`AC-4` (Тикет #3 — каталог моделей и `--list_models`):**
  - Доступен каталог метаданных моделей (содержащий `name`, `filename`, `architecture`, `stems`, опциональный `SDR`, `checksum` и пояснение, как получить файл весов), поставляемый с CLI и/или загружаемый из пользовательского манифеста;
  - реализованы флаги `--list_models`, `--list_filter`, `--list_limit`, `--list_format pretty|json`;
  - режим каталога работает исключительно с метаданными без сетевых загрузок, имеет документированные колонки для `pretty` и стабильную JSON-схему для `json`.
- **`AC-5` (Тикет #4 — диагностика CLI: `--env_info`, `--debug`/`--log_level`, тайминги):**
  - Подключён бэкенд логирования (`tracing`/`log`) с флагом `--log_level`, алиасом `--debug`, временными метками, целевыми модулями (module targets) и уровнем по умолчанию `info`;
  - флаг `--env_info` работает без наличия файлов весов моделей, не обращается к сети, не выводит секретов и сообщает версии крейта и CLI, ревизию/версию Candle, скомпилированные фичи (`cuda`/`metal`), результат проверки доступности вычислительного устройства, доступный параллелизм и разрешённые пути к моделям;
  - ошибки содержат путь к входному файлу и причину;
  - подробный режим (`debug`/verbose) выводит длительность стадий (`decode`, `separate`, `write`, `transcribe`), не изменяя стандартный вывод в режиме по умолчанию.
- **`AC-6` (Тикет #5 — многостемовый вывод и `--single_stem <name>`):**
  - Публичный библиотечный API предоставляет набор именованных стемов (не ограниченный двумя фиксированными полями), сохраняя удобный доступ для двухстемового сценария (`Vocals`/`Instrumental`), и для 4-стемовой модели возвращает четыре именованных `AudioBuffer`;
  - семантика остатка (residual) применяется только там, где вторичный стем означает «остаток микса», тогда как многостемовые модели возвращают независимые стемы без требования равенства их суммы входу;
  - в CLI флаг `--single_stem <name>` выбирает конкретный именованный стем, и формируются корректные имена выходных файлов для каждого стема.
- **`AC-7` (Тикет #6 — выходные кодеки поверх WAV f32: FLAC, MP3, решение по M4A/AAC, битрейт, частота дискретизации):**
  - Реализованы флаги `--output_format`, `--output_bitrate`, `--sample_rate` (с переиспользованием существующей логики ресемплинга);
  - формат WAV по умолчанию остаётся `f32` без скрытого дизеринга;
  - форматы FLAC и MP3 кодируются и проходят обратное чтение собственным декодером (`read_audio`) с совпадением длительности и частоты дискретизации с эталоном (в пределах допусков формата/фрейминга кодека);
  - неподдерживаемые форматы (включая документированное решение по отказу/ограничению для M4A/AAC при отсутствии чистого Rust/offline-энкодера), неподдерживаемые битрейты и некорректные частоты дискретизации явно отвергаются с понятной ошибкой;
  - все новые зависимости задокументированы в `docs/dependency-audit.md`.
- **`AC-8` (Тикет #7 — обработка уровней: нормализация, усиление, спектральная инверсия):**
  - Реализована пиковая нормализация входа и выхода к настраиваемому порогу (по умолчанию `0.9`);
  - реализовано усиление тихих входов ниже заданного порога (по умолчанию `0.0` = выключено);
  - реализован флаг `--invert_spect` для вычисления вторичного стема спектральной инверсией вместо вычитания волны во временной области;
  - инвариант точности остатка (`vocals + instrumental = input` в режиме точного остатка без независимой перенормализации стемов vs поведение в нормализованном режиме) явно зафиксирован и соблюдается в обоих режимах, а флаги CLI и их значения по умолчанию соответствуют референсной семантике.
- **`AC-9` (Документация и автокоммит):**
  - `README.md` (и сопутствующая документация репозитория) обновлены в соответствии с новой функциональностью тикетов `#1`–`#7` и версией `candle-roformer` `1.0`;
  - все изменения закоммичены локально в git-репозитории `/home/alex/sources/karaoke-cli` (рабочее дерево чистое, `git push` не выполнялся, сторонние файлы и другие репозитории не затронуты).

---

## 3. Метод измерения (для числовых и инвариантных условий тикетов)

Задача в целом носит функционально-контрактный характер (дискретное выполнение `AC-1`–`AC-9`). Для числовых параметров, входящих в состав тикетов `#6` и `#7`, действуют следующие спецификации измеримости и сопоставимости:

- **Совпадение частоты дискретизации и длительности при round-trip кодировании (`AC-7`, тикет #6):**
  - *Метрика и единица:* частота дискретизации декодированного файла (Гц, точное совпадение с целевым `--sample_rate` или исходной частотой) и длительность декодированного аудиосигнала в секундах (отношение числа сэмплов на канал к частоте дискретизации).
  - *Знаменатель и фильтры:* синтетический или фикстурный аудиосигнал известной длины и частоты дискретизации, записанный энкодером (`WAV`, `FLAC`, `MP3`) и прочитанный через `read_audio`.
  - *Условия сопоставимости и порог:* для `WAV` и `FLAC` (lossless) — точное совпадение частоты дискретизации и числа сэмплов; для `MP3` (lossy с фреймовым паддингом LAME/декодера) — точное совпадение частоты дискретизации и совпадение длительности в пределах одного-двух аудиофреймов MP3 ($\le 0.05\text{ с}$). Происхождение порога: требование тикета `#6` («match the reference duration/sample rate») с учётом размера фрейма MP3 (1152 сэмпла).
- **Пороги уровня и инвариант остатка (`AC-8`, тикет #7):**
  - *Метрика и единица:* пиковая амплитуда $\max_i |x_i|$ (безразмерная величина в линейной шкале `f32`) и поточечная погрешность восстановления $\max_i |(v_i + inst_i) - x_i|$.
  - *Условия сопоставимости и порог:* порог пиковой нормализации по умолчанию равен `0.9` (при включённой нормализации пик результата совпадает с целевым порогом в пределах погрешности `f32` $\le 10^{-5}$); порог усиления тихих сигналов по умолчанию равен `0.0` (выключено); в режиме точного остатка (без независимой пиковой перенормализации стемов и без `--invert_spect`) сумма `vocals + instrumental` восстанавливает входной сигнал с точностью арифметики плавающей точки `f32` ($\le 10^{-5}$). Происхождение порогов: спецификация тикета `#7`.

---

## 4. Ограничения и допущения

### Ограничения задачи и среды (Constraints)

- **Автономный режим:** без интерактивных вопросов пользователю; завершение только при полном подтверждении критериев приёмки или доказанном неустранимом блокере.
- **Политика Git:** разрешены `git add` и `git commit` только внутри `/home/alex/sources/karaoke-cli`; категорически запрещены `git push`, переписывание опубликованной истории, индексация посторонних файлов и любые изменения вне `/home/alex/sources/karaoke-cli`.
- **Политика GitHub Issues:** не закрывать и не комментировать тикеты на GitHub (`gh issue close` / `gh issue comment` запрещены); только локальные коммиты.
- **Защита конфигурации хоста:** запрещено редактировать `/home/alex/.config/opencode/**`.
- **Автономность тестов:** тесты не должны требовать скачивания многогигабайтных реальных весов моделей из сети (`--env_info`, `--list_models`, обработка аудио, ошибок, кодеков и многостемового API должны проверяться на локальных синтетических данных/фикстурах/минимальных графах весов).

### Известные ресурсные ограничения и ограничения доступа (Known resource and access constraints)

- Локальный тулчейн Rust (`cargo`), git и доступ к сети (crates.io для разрешения `candle-roformer = "1.0"` и крейтов-кодеков `flacenc`, `mp3lame-encoder` и др., если требуются в `karaoke-cli`) доступны и функционируют.
- В рабочей директории присутствуют незакоммиченные правки предыдущего шага в `Cargo.toml`, `Cargo.lock` и `README.md` (переключение с локального пути на опубликованный `candle-roformer`), которые входят в контур обновления до `1.0`.

### Допущения (Assumptions)

- **A-1:** В `candle-roformer` `1.0.0` уже присутствуют примитивы, закрывшие апстрим-тикеты (многостемовый API, спектральная инверсия / нормализация / ресемплинг / чтение аудио и др.), либо часть CLI-обвязки (каталог моделей, `--custom_output_names`, пакетный обход директорий, логирование `--env_info`/`--log_level`, выходные кодеки и `docs/dependency-audit.md`) реализуется непосредственно в `karaoke-cli` поверх публичного API `candle-roformer` `1.0.0`.
- **A-2:** По формату M4A/AAC (тикет `#6`: «a decision for M4A/AAC»): поскольку в экосистеме Rust нет чистого memory-safe встроенного AAC/M4A-энкодера без внешних системных библиотек/FFmpeg, явный детерминированный отказ для `--output_format m4a`/`aac` с понятным сообщением об ошибке и фиксацией этого архитектурного решения в документации и `docs/dependency-audit.md` полностью удовлетворяет требованию тикета `#6` (при условии, что `candle-roformer` 1.0 сам не предоставляет готовый M4A-энкодер; если предоставляет — переиспользовать его).
- **A-3:** Имена флагов CLI, указанные в тикетах с подчёркиваниями (`--custom_output_names`, `--list_models`, `--list_filter`, `--list_limit`, `--list_format`, `--env_info`, `--log_level`, `--single_stem`, `--output_format`, `--output_bitrate`, `--sample_rate`, `--invert_spect`), должны поддерживаться в точном виде с подчёркиваниями (при необходимости с kebab-case алиасами в `clap`), чтобы строго соответствовать формулировкам тикетов `#2`–`#7`.

---

## 5. Закрытие пробелов (Gap closure)

| Пробел в постановке (Statement gap) | Способ закрытия (Evidence / Assumption / Blocker) |
| --- | --- |
| Точный состав публичного API `candle-roformer` `1.0.0` относительно того, какая доля логики тикетов `#1`–`#7` уже вынесена в библиотечный крейт `candle-roformer`, а какая остаётся на стороне `karaoke-cli` | **Доказательство на входе DO / Допущение A-1:** `candle-roformer` `1.0.0` опубликован на crates.io; после обновления `Cargo.toml` до `1.0` DO напрямую использует экспортируемые типы/функции `candle-roformer` `1.0` и достраивает недостающий CLI/библиотечный слой в `karaoke-cli`. Это не меняет запрашиваемый результат и критерии `AC-1`–`AC-9`. |
| Поддержка кодирования M4A/AAC в тикете `#6` («a decision for M4A/AAC») | **Доказательство из текста тикета + Допущение A-2:** Тикет `#6` прямо требует «a decision for M4A/AAC» и «FLAC and MP3 round-trip through our own decoder (`read_audio`)... `--output_format`, `--output_bitrate`, `--sample_rate` exist and are rejected clearly when unsupported; new deps added to `docs/dependency-audit.md`. Явная поддержка WAV/FLAC/MP3 и чистый диагностический отказ для M4A/AAC (с документированием решения в `docs/dependency-audit.md` и `README.md`) закрывают требование без изменения критериев приёмки. |
| Рекурсивность обхода директорий в тикете `#1` («non-recursive?, stable ordering») | **Допущение в средствах реализации:** Нерекурсивный обход файлов верхнего уровня указанной директории с фильтрацией по поддерживаемым аудиорасширениям и лексикографической сортировкой путей обеспечивает предсказуемый, безопасный и детерминированный пакетный запуск. |
| Гранулярность автокоммитов («автокомит») | **Допущение в средствах реализации:** Локальные коммиты в ветке `main` (по логическим блокам/тикетам или единой серией атомарных коммитов с чистым рабочим деревом по завершении) без `git push`. |

---

## 6. Минимальное решение и альтернативы

- **Выбранное минимальное решение:**
  1. Обновить зависимость `candle-roformer` в `Cargo.toml` до `1.0` и синхронизировать вызовы с API `1.0.0`.
  2. Расширить публичный библиотечный интерфейс `karaoke_cli` (`src/lib.rs`, `src/pipeline.rs`) и CLI (`src/main.rs`) для поддержки всех 7 тикетов:
     - многостемового вывода и `--single_stem <name>` (`#5`),
     - пользовательских имён выходных файлов и sidecar-файлов `--custom_output_names` с защитой от path traversal и валидацией имён стемов (`#2`),
     - пакетной обработки директорий с пофайловым preflight, продолжением при ошибках и сводным отчётом (`#1`),
     - обработки уровней (пиковая нормализация вх./вых. с порогом `0.9`, усиление тихих сигналов с порогом `0.0`, `--invert_spect`, соблюдение инварианта остатка) (`#7`),
     - выходных форматов (`--output_format wav|flac|mp3`, отказ для неподдерживаемых вроде `m4a`), `--output_bitrate`, `--sample_rate` и ведения `docs/dependency-audit.md` (`#6`),
     - встроенного/пользовательского каталога моделей с `--list_models`, `--list_filter`, `--list_limit`, `--list_format pretty|json` (`#3`),
     - логирования (`--log_level`, `--debug`) с таймингами стадий, обогащённого контекста ошибок (путь + причина) и флага `--env_info` (`#4`).
  3. Покрыть каждую новую возможность автоматическими тестами, обновить `README.md` и `docs/dependency-audit.md`, зафиксировать изменения локальными git-коммитами.
- **Альтернативы (отклонены):**
  - *Оформить задачу как `collection` из 7 изолированных циклов:* отклонено, так как все 7 тикетов модифицируют одни и те же структуры `Cli`, `RunConfig`/`SeparationOptions` и конвейер в 3 исходных файлах, а `#2`, `#5`, `#6` и `#7` напрямую пересекаются в логике именования и записи выходных стемов. Единый цикл `cycle` исключает промежуточные конфликты сигнатур.

---

## 7. Оценка трудоёмкости (Effort estimate)

- Режим **автономный** (оценка приводится справочно, без ограничения времени/шагов выполнения): средний объём работ в рамках одного крейта (обновление зависимости до `1.0`, расширение 3 модулей в `src/`, добавление каталога/аудита зависимостей и интеграционных тестов в `tests/`). Пробный шаг (bounded probe) перед DO не требуется, так как все механизмы стандартны и опираются на опубликованный `candle-roformer` `1.0.0`.

---

## 8. Передача из PLAN в DO (Plan-to-DO handoff)

Каждый запуск DO в рамках этого цикла получает по ссылке следующий неизменный контракт:

- **Исходная задача:** «вышла новая версия 1.0, там закрыты все тикеты, которые нужны были для реализации открытых тикетов проекта / запускай pdca по открытым тикетам автономно автокомит».
- **Обязательные критерии приёмки:** `AC-1`–`AC-9` в полном объёме.
- **Область действия (Scope):** Репозиторий `/home/alex/sources/karaoke-cli` (код `src/**`, тесты `tests/**`, манифест `Cargo.toml`/`Cargo.lock`, данные каталога моделей, `README.md`, `docs/dependency-audit.md`, локальные git-коммиты).
- **Запреты (Prohibitions):**
  - Запрещено выполнять `git push`, переписывать историю git или трогать любые файлы вне `/home/alex/sources/karaoke-cli`.
  - Запрещено закрывать, редактировать или комментировать тикеты в GitHub.
  - Запрещено редактировать `/home/alex/.config/opencode/**`.
  - Запрещено ослаблять существующие тесты или оставлять рабочее дерево грязным по завершении DO.
- **Свобода реализации DO и проверки CHECK:** Декомпозиция на внутренние рабочие единицы, порядок реализации тикетов и структура коммитов принадлежат DO; выбор конкретных команд верификации и источников доказательств принадлежит CHECK во время фазы CHECK.

---

## 9. Риски и условия остановки (Risks and stop conditions)

- **Риски:**
  - **R-1 (Несовместимость сигнатур `candle-roformer` 1.0):** В версии `1.0.0` могли измениться имена типов, полей конфигурации или сигнатуры функций разделения/транскрипции/ресемплинга по сравнению с `0.1`. *Митигация:* на этапе DO свериться с фактическими экспортами `candle-roformer` `1.0.0` и адаптировать обёртки в `src/lib.rs` и `src/pipeline.rs`.
  - **R-2 (Внешние си-зависимости для аудиокодеков):** Крейт `mp3lame-encoder` компилирует C-код LAME и требует рабочего C-компилятора в окружении (либо использования уже экспортируемых средств `candle-roformer` 1.0, если кодирование реализовано там). *Митигация:* в первую очередь проверить, экспортирует ли `candle-roformer` `1.0.0` готовые функции записи/кодирования аудио и обработки уровней; при подключении прямых зависимостей в `karaoke-cli` зафиксировать их в `docs/dependency-audit.md`.
  - **R-3 (Конфликт типов флага `--single_stem`):** В существующем CLI `--single_stem` мог быть булевым флагом, тогда как тикет `#5` требует `--single_stem <name>`. *Митигация:* перевести `--single_stem` на приём строкового имени стема (например, `Vocals`, `Instrumental`, `drums`, `bass`, `other` без учёта регистра или с чёткой валидацией) и обновить существующие тесты/документацию.
- **Условия остановки (Stop conditions):**
  - **Успешное завершение:** Все критерии `AC-1`–`AC-9` независимо подтверждены на фазе CHECK.
  - **Эскалация / возврат в PLAN (CHECK → PLAN):** Обнаружено объективное противоречие между фактическим публичным API `candle-roformer` `1.0.0` и требованиями тикетов `#1`–`#7`, которое невозможно разрешить средствами `karaoke-cli` без изменения контракта.
  - **Лимит попыток:** Исчерпание бюджета попыток (`n = 3`) по одному и тому же ключу дефекта без прогресса переводит цикл на `pdca-escalate`.

---

## 10. Ревизия контракта (Contract revision)

- Первичный план (`r = 1`), ревизий контракта не производилось.

---

## 11. Уверенность и пробелы (Confidence and gaps)

- **Подтверждено фактами (Verified):** Наличие ровно 7 открытых тикетов (`#1`–`#7` с чёткими критериями `Done when:` в каждом); публикация `candle-roformer` `1.0.0` на crates.io; структура проекта (`src/lib.rs`, `src/main.rs`, `src/pipeline.rs`, `tests/flow.rs`, `README.md`); работоспособность `cargo test` и доступность локального git-репозитория с разрешённым автокоммитом.
- **Остаётся допущением (Assumptions):** Допущения `A-1`–`A-3` касаются исключительно внутренних средств реализации поверх `candle-roformer` `1.0.0` и не меняют запрашиваемый пользователем результат. Блокеров для перехода в фазу DO нет (`status: ready`).
````

## Approval record

`not applicable: autonomous mode`

## DO result

DO started 2026-11-13T19:54:43Z after re-anchoring the approved worktree at `/home/alex/sources/karaoke-cli`, branch `main`, baseline `a1ea0a4cea0d7d19142531d91e91469e731af2e8`. The embedded frozen plan report matched the supplied SHA-256 `534dcb4964ef6ecb072be2a250b3666921bb003894f6879d4d3da97467bb5699` (178 lines, 33,999 bytes); all original task anchors exist at their planned paths. Existing dirty paths before DO: `Cargo.toml`, `Cargo.lock`, `README.md`, and pre-existing untracked `.pdca/status/karaoke-cli-open-issues.md`, `.pdca/status/karaoke-cli-open-issues/planner-plan-r1.md`, `.pdca/status/karaoke-cli-open-issues/preparation-r1.md`; these are outside current task ownership except the three dependency/docs files explicitly included by this plan, and the unrelated status files remain untouched.

Execution units (serial, shared CLI/pipeline ownership):
- `U1` / `AC-1`: published dependency v1.0 and actual public API alignment — `done` (`cargo test --no-run` exit 0, 3 test binaries; API fixture adapted; log `.pdca/open-issues/rv1/attempts/cargo-test-no-run-api-fixture-fixed.log`).
- `U2` / `AC-2`: input-directory discovery, per-file preflight, continue-on-error and summary — `done` (targeted integration test exit 0, 1 passed; mixed-batch CLI expected-negative exit 1, 3 inputs selected, 1 succeeded, 2 failed; existing rejected output preserved and the later valid file produced 2 stems; logs `.pdca/open-issues/rv1/attempts/cargo-test-ticket1-preflight-continue.log` and `.pdca/open-issues/rv1/attempts/cli-ticket1-per-file-preflight.log`).
- `U3` / `AC-3` (ticket #2): validated custom output names and transcription sidecars — `done`; targeted flow test exit 0 (1 passed), CLI custom-name run exit 0 (1 input succeeded and custom Vocal/default Instrumental outputs), transcription CLI run after Whisper asset-order fix exit 0 (1 input succeeded; `.wav`, `.txt`, and `.json` share `cli-transcript-name`), malformed JSON CLI expected-negative exit 2 with no output directory, and traversal CLI expected-negative exit 1 with no output directory. Evidence: `.pdca/open-issues/rv1/attempts/cargo-test-ticket2-custom-names-after-assertion-fix.log`, `.pdca/open-issues/rv1/attempts/cargo-test-ticket2-json-cli-qualified.log`, `.pdca/open-issues/rv1/attempts/cli-ticket2-custom-name.log`, `.pdca/open-issues/rv1/attempts/cli-ticket2-custom-sidecars-after-asset-order-fix.log`, `.pdca/open-issues/rv1/attempts/cli-ticket2-reject-json.log`, `.pdca/open-issues/rv1/attempts/cli-ticket2-reject-traversal.log`; committed as `8920434c6afac4457006dbeeace989972384f3e5`.
- `U4` / `AC-4` (ticket #3): metadata model catalog and listing CLI — `done`; offline fixture test exit 0 (1 selected/passed; filter and limit, pretty columns, stable JSON key schema); offline CLI JSON run exit 0 selecting exactly 1 of 3 fixture models using case-insensitive filter and limit; built-in pretty CLI run exit 0 listing 3 local rows. No weights or network access used. Evidence: `.pdca/open-issues/rv1/attempts/cargo-test-ticket3-catalog-fixture.log`, `.pdca/open-issues/rv1/attempts/cli-ticket3-manifest-filter-limit-json.log`, `.pdca/open-issues/rv1/attempts/cli-ticket3-builtin-pretty.log`; committed as `74b3e76146ae0e40a196ad124b10c13c7b5fb73a`.
- `U5` / `AC-5` (ticket #4): diagnostics, environment report, contextual errors and stage timings — `done` for the attempt-2 implementation candidate on the full-suite and direct `--env_info` evidence below; independent CHECK remains pending. The false dependency revision field was removed and lock-derived version/source reporting was added. Earlier evidence: `.pdca/open-issues/rv1/attempts/cargo-test-ticket4-cli-unit.log`, `.pdca/open-issues/rv1/attempts/cli-ticket4-env-info-no-weights-after-rebuild.log`, `.pdca/open-issues/rv1/attempts/cli-ticket4-env-info-resolved-paths-after-rebuild.log`, `.pdca/open-issues/rv1/attempts/cli-ticket4-default-output.log`, `.pdca/open-issues/rv1/attempts/cli-ticket4-debug-stage-timings.log`, `.pdca/open-issues/rv1/attempts/cli-ticket4-error-path-cause.log`.
- `U6` / `AC-6` (ticket #5): named multi-stem API/CLI and single-stem selection — `done`; actual `candle-roformer 1.0.0` `Stems` returned through public `karaoke_cli::separate_audio` / `NamedStems`; synthetic 4-stem profile returned `drums`, `bass`, `other`, `vocals`, preserved named estimates (no `instrumental` residual), and convenience fields (`vocals`; fallback `instrumental`→`bass`); synthetic 1-stem API retained `vocals`/`instrumental` and exact residual reconstruction within `1e-4`; CLI emitted all four correctly named outputs and `--single_stem bass` emitted only `song_(Bass)_tiny_sep.wav`. Offline test exit 0: 1 selected/passed. Commit `253e07a776e0449f12ab0923c3cf224cb18afc1e`; evidence `.pdca/open-issues/rv1/attempts/cargo-test-ticket5-four-stem-and-residual.log`.
- `U7` / `AC-7` (ticket #6): WAV/FLAC/MP3 output, bitrate/sample-rate validation and dependency audit — `done`; WAV round-trip preserved exact f32 samples; FLAC exact sample count and MP3 duration within 0.05 s through `read_audio`; exact sample rate verified at 44.1 kHz and at CLI-selected 48 kHz after resampling; M4A/AAC and invalid bitrate/rate rejection covered; synthetic CLI emitted FLAC and MP3 files, MP3 used 128 kbps. Targeted offline runs: codec unit tests 5 passed, CLI flag parser 1 passed, CLI codec/resample integration 1 passed after FLAC short-frame fix. Dependency audit and codec documentation added. Commit `1f98eb4deee4e0709e227ebe4ece376a4eeeec9f`. Evidence: `.pdca/open-issues/rv1/attempts/cargo-test-ticket6-output-tests-after-blocksize-fix.log`, `.pdca/open-issues/rv1/attempts/cargo-test-ticket6-cli-flags.log`, `.pdca/open-issues/rv1/attempts/cargo-test-ticket6-cli-codecs-after-blocksize-fix.log`, `.pdca/open-issues/rv1/attempts/dependency-direct-versions.log`, `.pdca/open-issues/rv1/attempts/git-commit-ticket6.log`.
- `U8` / `AC-8` (ticket #7): normalization, quiet-input gain and spectral inversion/residual behavior — `done`; CLI and `InferenceOverrides` defaults verified (`0.9`, `0.0`, `false`); synthetic exact-residual max error ≤`1e-5`; normalized mode verifies independent non-silent stem peaks at 0.9 and input-scale invariance; amplification test verifies quiet input is raised to 0.1 reference; inversion output matches the upstream complex-STFT residual. Targeted offline tests passed 1/1 integration and 1/1 CLI parser test; vacuous filter defect resolved. Commit `e4fe915b1c2fc1834f639974250e5f450e3ea3b4`. Evidence: `.pdca/open-issues/rv1/attempts/cargo-test-ticket7-level-semantics.log`, `.pdca/open-issues/rv1/attempts/cargo-test-ticket7-cli-level-flags-qualified.log`, `.pdca/open-issues/rv1/attempts/git-commit-ticket7.log`.
- `U9` / `AC-9`: user documentation, local commits and clean final worktree — `done` on the post-commit Git evidence below; product changes were already committed, and all remaining `.pdca/` cycle evidence plus the three superseded, never-run preparation records were committed together as the durable PDCA record. No `.gitignore`/ignore-rule workaround and no deletion.

The approved `Cargo.toml` dependency update is applied (`candle-roformer = "1.0"`). First cargo-update invocation exited `127` because `/home/alex/.cargo/bin` is not on this shell's `PATH`; Cargo did not launch. The exact attempt log is `.pdca/open-issues/rv1/attempts/cargo-update-candle-roformer.log`. Explicit `/home/alex/.cargo/bin/cargo update -p candle-roformer --precise 1.0.0` then exited `0`, resolving `candle-roformer 0.1.0 → 1.0.0` and adding 44 dependency packages; full log `.pdca/open-issues/rv1/attempts/cargo-update-candle-roformer-1.0.0.log`. `cargo fetch` exited `0`, downloaded `candle-roformer v1.0.0` (plus 6 packages logged); full log `.pdca/open-issues/rv1/attempts/cargo-fetch-1.0.0.log`. Inspection of the registry crate source confirms `lib.rs` exports `Separator`, `Stems`, `InferenceConfig`, `AudioBuffer`, decode/WAV functions; `Stems.stems` exposes arbitrary named stems; `InferenceConfig` defaults normalization `0.9`, amplification `0.0`, invert false; `resample` module exists. The published crate only writes WAV via `write_wav_f32`; it exports no FLAC/MP3 encoder. Relevant source snapshot and its checksums are persisted under `.pdca/open-issues/rv1/source/candle-roformer-1.0.0/` for CHECK. `cc` 11.4.0 exists; LAME/FLAC pkg-config tools were not found in the initial probe. `cargo search flacenc --limit 5` and `cargo search mp3lame-encoder --limit 5` both exited `0`; logs are `.pdca/open-issues/rv1/attempts/cargo-search-flacenc.log` and `.pdca/open-issues/rv1/attempts/cargo-search-mp3lame.log`. Search found `flacenc 0.5.1` (pure Rust) and `mp3lame-encoder 0.3.0`. Initial `cargo test --no-run` invocation exited `127` before cargo launch because `timeout` attempted to execute `CARGO_BUILD_JOBS=1` as a command; log `.pdca/open-issues/rv1/attempts/cargo-test-no-run-upstream-v1.0.log`. The corrected command uses `env CARGO_BUILD_JOBS=1`. A corrected `cargo test --no-run` with 8 GiB virtual-memory cap and 600-second timeout first exited `101` in `libsamplerate-sys 0.1.12`: its bundled CMake project uses `cmake_minimum_required(<3.5)`, rejected by installed CMake `4.4.4`; full log `.pdca/open-issues/rv1/attempts/cargo-test-no-run-upstream-v1.0-corrected.log`. Setting `CMAKE_POLICY_VERSION_MINIMUM=3.5` overcame that native configure issue but the next compile exited `101` because `candle-roformer` 1.0 added `family` and `freqs_per_bands` fields to the `SeparatorConfig` test fixture at `tests/flow.rs:130`; full log `.pdca/open-issues/rv1/attempts/cargo-test-no-run-cmake-policy-min-3.5.log`. The fixture now sets both to `None`; a repeat `cargo test --no-run` with the 8 GiB virtual-memory cap, 600-second timeout, one build job and CMake policy environment exited `0`, compiling all targets and creating 3 test executables; log `.pdca/open-issues/rv1/attempts/cargo-test-no-run-api-fixture-fixed.log`. `cargo info flacenc@0.5.1` and `cargo info mp3lame-encoder@0.3.0` exited `0`; their logs are `.pdca/open-issues/rv1/attempts/cargo-info-flacenc.log` and `.pdca/open-issues/rv1/attempts/cargo-info-mp3lame-encoder.log`. `cargo info mp3lame-sys@0.2.0` also exited `0` (log `.pdca/open-issues/rv1/attempts/cargo-info-mp3lame-sys.log`). Its source shows bundled static LAME v4.0 C source compiled by `cc` (`build.rs` and README snapshot under `.pdca/open-issues/rv1/source/mp3lame-sys-0.2.0/`), so system `libmp3lame`/pkg-config is not required; C compiler `cc 11.4.0` is present. `flacenc` is pure Rust (Apache-2.0), the MP3 wrapper/sys crates are LGPL-3.0, and codec sources/metadata are snapshotted under `.pdca/open-issues/rv1/source/{flacenc-0.5.1,mp3lame-encoder-0.3.0,mp3lame-sys-0.2.0}/`. Full test suite and actual codec build/round-trip remain to be run. Evidence root: `.pdca/open-issues/rv1/`.

Re-anchor for the current continuation confirmed the #6 commit `1f98eb4deee4e0709e227ebe4ece376a4eeeec9f` and the user-supplied uncommitted #7 changes; no semantic or cosmetic drift. The level semantics/docs and residual tests are now committed as `e4fe915b1c2fc1834f639974250e5f450e3ea3b4`. The fully qualified CLI rerun selected/passed 1 test, resolving the zero-selection attempt. The remaining authorized actions are full unfiltered `cargo test`, final `cargo fmt --check`, status finalization/commit, and final Git-state accounting. Same DO attempt `N=1, r=1, n=1/3`.

Continuation request (verbatim):

> Continue the same DO attempt (N=1, r=1, n=1/3). Units #1–#5 are committed and closed.
>
> Scope for this message only:
> 1. Implement, test and commit unit #6 (ticket #6): `--output_format wav|flac|mp3` (WAV stays f32 default, no hidden dithering), `--output_bitrate` for lossy, `--sample_rate` reusing the resample module; FLAC and MP3 round-trip through `read_audio` (exact sample rate; duration tolerance ≤0.05 s for MP3, exact samples for WAV/FLAC); unsupported formats including M4A/AAC rejected with a clear documented error; every new dependency recorded in `docs/dependency-audit.md`. Synthetic fixtures only.
> 2. Implement, test and commit unit #7 (ticket #7): peak normalization of input/output with default 0.9, amplification of quiet inputs with default 0.0 = off, `--invert_spect` for the secondary stem, and a precise residual-invariant statement: exact-residual mode restores `vocals+instrumental=input` within f32 tolerance ≤1e-5; normalized mode documented and tested for its own invariant. CLI flags/defaults match the reference semantics.
> 3. Update `README.md` and `docs/dependency-audit.md` for everything implemented in #1–#7, persist logs, update status after each unit, commit each unit.
> 4. Then run the full `cargo test`, persist the log, commit any remaining doc/status changes, and report: commit hashes, evidence paths, exact test counts, any residual unmet/unverified AC, and the final git state.
>
> If a codec crate cannot build in this environment, record the precise blocker and evidence; fall back to a clearly documented unsupported error only if the plan allows it, and do not silently weaken the AC.

Continuation request (verbatim):

> Continue the same DO attempt (N=1, r=1, n=1/3). Unit #6 is committed; #7 has uncommitted working changes.
>
> Scope for this message:
> 1. Finish unit #7: resolve the vacuous-test defect entry in the status file, complete the README/docs for #7 and update the now-stale transcription/test descriptions, then commit #7.
> 2. Run the FULL `cargo test` (not a filtered run), persist the log, and confirm the exact counts and exit status.
> 3. Verify `cargo fmt --check` (or the repo's convention) and that the working tree is clean after committing all authorized doc/status changes.
> 4. Update the status file with the DO result: changed files, commit hashes, evidence paths, per-AC status (met/unmet/unverified as your candidate assessment, not a verdict), and final git state.
> 5. Return the DO report: completed candidate, commit list, exact test counts, evidence pointers, residual issues, and confirmation that no push happened and no files outside the repo changed.
>
> Do not issue PASS/verdict; that is CHECK's job. If the full suite fails, fix the failure first and re-run before reporting.

Final DO result (candidate assessment only; CHECK not run): the full, unfiltered `cargo test` exited `0`: library unit tests 7/7, CLI unit tests 6/6, integration tests 8/8, doc-tests 0 (none defined), total 21 passed, 0 failed. Full log: `.pdca/open-issues/rv1/attempts/cargo-test-full-final.log`. `cargo fmt --all -- --check` exited `0`; log: `.pdca/open-issues/rv1/attempts/cargo-fmt-check-final.log`. Candidate assessment: `AC-1`–`AC-8` met on DO evidence, pending independent CHECK; `AC-9` unmet because the final worktree is not clean. Product commits are `1ed3cb908dea932196fcef99fbbbfd96135cac84`, `8920434c6afac4457006dbeeace989972384f3e5`, `74b3e76146ae0e40a196ad124b10c13c7b5fb73a`, `8345e3b885e2324a431bcd61068eec0257205a1d`, `253e07a776e0449f12ab0923c3cf224cb18afc1e`, `1f98eb4deee4e0709e227ebe4ece376a4eeeec9f`, and `e4fe915b1c2fc1834f639974250e5f450e3ea3b4`. The status file was first committed locally as `9a218e2205c762a312df00336923f69aa34190fb`; full log `.pdca/open-issues/rv1/attempts/git-commit-cycle-status.log`. Post-commit inspection observed `main...origin/main [ahead 8]`, with `.pdca/open-issues/` and the three unrelated pre-existing status paths untracked; the observation is recorded here before the authorized final ledger-correction commit. Evidence and unrelated records remain untracked. No push and no writes outside this repository. U9 remains blocked; route the incomplete DO candidate to ACT without CHECK.
Final DO result (candidate assessment only; CHECK not run): the full, unfiltered `cargo test` exited `0`: library unit tests 7/7, CLI unit tests 6/6, integration tests 8/8, doc-tests 0 (none defined), total 21 passed, 0 failed. Full log: `.pdca/open-issues/rv1/attempts/cargo-test-full-final.log`. `cargo fmt --all -- --check` exited `0`; log: `.pdca/open-issues/rv1/attempts/cargo-fmt-check-final.log`. Candidate assessment at that point: `AC-1`–`AC-8` met on DO evidence; `AC-9` was unmet while `.pdca/` remained untracked. Product commits are `1ed3cb908dea932196fcef99fbbbfd96135cac84`, `8920434c6afac4457006dbeeace989972384f3e5`, `74b3e76146ae0e40a196ad124b10c13c7b5fb73a`, `8345e3b885e2324a431bcd61068eec0257205a1d`, `253e07a776e0449f12ab0923c3cf224cb18afc1e`, `1f98eb4deee4e0709e227ebe4ece376a4eeeec9f`, and `e4fe915b1c2fc1834f639974250e5f450e3ea3b4`. The status file was first committed locally as `9a218e2205c762a312df00336923f69aa34190fb`; full log `.pdca/open-issues/rv1/attempts/git-commit-cycle-status.log`. Post-commit inspection observed `main...origin/main [ahead 8]`, with `.pdca/open-issues/` and the three preparation records untracked; the final ledger correction was committed in `92150b8bbe8b0aee77a1e68766a2c0b3e036290e`. This continuation resolves the remaining dirty state by preserving and committing the full `.pdca/` record, not by changing `.gitignore`, ignoring files, or deleting files.

Continuation request (verbatim):

````text
Resolve the AC-9 decision as follows; this is not a scope expansion.

- Do NOT touch `.gitignore` and do NOT use ignore rules. `.pdca/` is the cycle's own durable state and its status file is already tracked/committed in this cycle; committing the remaining `.pdca` state is the same contour, and AC-9 explicitly requires all changes committed and a clean tree.
- The three `karaoke-cli-open-issues*` files are NOT foreign user files: they are preparation records of the superseded, never-run orchestrator attempt at this same task. Preserve them by committing them with the rest, not by deleting or ignoring them.
- Use ONE commit for the remaining `.pdca/` content so the final count is `ahead 10`, not 11:
  1. Update `.pdca/status/open-issues-1.md` first: AC-9 resolution (all product changes committed; remaining cycle evidence and the superseded preparation records are committed as the durable PDCA record; no .gitignore/ignore-rule workaround; no deletion), exact final test counts, final git state stated as "after this commit: clean working tree, main ahead of origin/main by 10, no push".
  2. `git add` the remaining `.pdca/` paths (the status file, `rv1/**`, and the three preparation records) and make one commit, e.g. `docs: persist PDCA cycle evidence and superseded preparation records`.
  3. Verify `git status --porcelain` is empty, `git status` shows ahead 10 and no push; persist the outputs under `.pdca/open-issues/rv1/attempts/` (this new evidence file will itself be untracked — if so, it must be included in the commit too; stage it before committing, or write the log before staging).
  4. Report: the single commit hash, the exact `git status` output, and confirmation of no push, no issue closing, and no changes outside the repo.

This closes DO. Do not run CHECK or issue a verdict.
````

Continuation DO update: the re-anchor found HEAD `92150b8bbe8b0aee77a1e68766a2c0b3e036290e`, `origin/main` `a1ea0a4cea0d7d19142531d91e91469e731af2e8`, branch `main` ahead 9, and 108 untracked paths with zero outside `.pdca/`; `.pdca/status/open-issues-1.md` is already tracked. The exact suite result remains 21 passed, 0 failed (library 7, CLI 6, integration 8; doc-tests 0); `cargo test` exit 0, log `.pdca/open-issues/rv1/attempts/cargo-test-full-final.log`. `git diff --cached --check` returned exit 2 because preserved older logs/snapshots have blank lines at EOF and the superseded preparation records have trailing whitespace; these records are being committed unchanged per instruction. The final Git state specified by this continuation is after one commit: clean tree, `main` ahead of `origin/main` by 10, no push. AC-9 closure and final evidence remain pending that commit and its post-commit verification.

AC-9 closure (candidate assessment, not CHECK): one local commit now contains the remaining `.pdca/` record, including this tracked status file, `rv1/**` evidence/snapshots, and all three superseded preparation records. The preparation files were preserved byte-for-byte, not deleted or ignored. No `.gitignore` change or ignore rule was made. Exact full-suite result: `cargo test` exit 0; 7 library + 6 CLI + 8 integration tests passed (21 passed, 0 failed), 0 doc-tests defined; log `.pdca/open-issues/rv1/attempts/cargo-test-full-final.log`. Post-commit `git status --porcelain` exited 0 with empty stdout; exact output file `.pdca/open-issues/rv1/attempts/git-status-porcelain-final.log` (empty file). Post-commit `git status` exited 0 and reported branch `main` ahead of `origin/main` by 10 with `nothing to commit, working tree clean`; exact output `.pdca/open-issues/rv1/attempts/git-status-final.log`, command metadata `.pdca/open-issues/rv1/attempts/git-status-final-metadata.log`. Those proof files are included in this same commit; the post-amend verification confirms the same clean status. Candidate assessment: AC-1–AC-9 met on DO evidence, pending independent CHECK. No push and no issue actions; no files outside the repository changed.

Continuation request (verbatim):

````text
PDCA DO, cycle N=1, r=1, attempt n=2/3 (CHECK→DO implementation defect; same revision, attempt incremented).

CHECK finding to fix, verbatim:
**ENV-PROVENANCE-R1-01 — major, confirmed; AC-5:** `build.rs` runs `git rev-parse` in the CLI's `CARGO_MANIFEST_DIR` and assigns the result to `CANDLE_ROFORMER_GIT_REVISION`. The CLI prints it as the dependency's revision. The old observed output `a1ea0a4cea0d` matches exactly this mistake. The green env-info test does not check the revision's provenance.

Required fix (CHECK's route): do not attribute the CLI's git HEAD to the dependency. Emit truthful dependency provenance, or correctly rename/remove the false field, while preserving the required versions (CLI/crate version, Candle version, features, device, parallelism, model paths).

Suggested minimal implementation:
- In `build.rs`, stop deriving any candle-roformer revision from this repo's git. Read the resolved dependency versions from `Cargo.lock` (parse the `candle-roformer` and `candle-core`/`candle-nn`/`candle-transformers` `[[package]]` entries) and emit them, e.g. `CANDLE_ROFORMER_VERSION` / `CANDLE_CORE_VERSION` plus source kind (`registry`), or read them via a robust equivalent. Do not claim a git revision for a registry build.
- In `--env_info`, print the dependency versions/source truthfully (e.g. `candle-roformer: 1.0.0 (crates.io)`, `candle-core: 0.11.1`) and remove/rename the misleading `revision` field. Keep the required candle version information from ticket #4.
- Add a regression assertion/test that the reported dependency version matches the `Cargo.lock` value and is not presented as the CLI repo's git HEAD (e.g. assert the emitted value equals the lock version and the label says registry/no git revision). Keep the existing env-info tests.

Then:
1. Run the full suite and fmt: `env CMAKE_POLICY_VERSION_MINIMUM=3.5 CARGO_BUILD_JOBS=1 /home/alex/.cargo/bin/cargo test --offline --locked` and `/home/alex/.cargo/bin/cargo fmt --all -- --check`; persist command/env/stdout/stderr/exit under `.pdca/open-issues/rv1/attempt-2/`.
2. After committing the fix and evidence, produce POST-COMMIT git evidence in that pack: `git rev-parse HEAD`, `git status --porcelain` (must be empty), `git rev-parse origin/main` (must still be a1ea0a4), `git log --oneline a1ea0a4..HEAD`, `git diff --stat a1ea0a4..HEAD -- . ':(exclude).pdca'`, and a statement on what cannot be audited.
3. Update `.pdca/status/open-issues-1.md`: attempt n=2/3, defect key ENV-PROVENANCE-R1-01 with fix, full test/fmt results, AC-9 post-commit clean-tree evidence and exact HEAD, final commits. Commit the status update.
4. Report: fix summary, commit hashes, exact test counts/exit, fmt exit, post-commit git facts, and any residual risk. Do not issue a verdict.

A later CHECK is the judge; do not declare PASS. If the build.rs fix is impossible without lying, remove the false field instead and document why.
````

Attempt n2 start: same approved revision r1; re-anchored at HEAD `ebb145264999aae589be3d6c8f79ee68b2afae90`, `origin/main` `a1ea0a4cea0d7d19142531d91e91469e731af2e8`, branch main ahead 10, initially clean. `build.rs` confirms the finding: it already reads package versions from Cargo.lock but also runs `git rev-parse` and publishes that repository HEAD as the dependency revision. The Cargo.lock entries are `candle-roformer 1.0.0` and `candle-core 0.11.1`, both from the crates.io registry. No source edit has yet been made; the stable defect key is `ENV-PROVENANCE-R1-01`.

Attempt n2 implementation: `build.rs` now extracts each unique Candle package's version and source kind from Cargo.lock, fails rather than emitting `unknown` if the package entry is missing/ambiguous, and no longer invokes Git or defines `CANDLE_ROFORMER_GIT_REVISION`. `--env_info` reports `candle-roformer version: 1.0.0 (crates.io registry)` and retains Candle core version, CLI/package version, feature/device/parallelism/path fields. The existing env-info unit test now compares the emitted Candle versions and source label directly to Cargo.lock and asserts the revision label is absent. Source delta: `build.rs`, `src/main.rs`.

Attempt n2 evidence: required offline locked full suite `env CMAKE_POLICY_VERSION_MINIMUM=3.5 CARGO_BUILD_JOBS=1 /home/alex/.cargo/bin/cargo test --offline --locked` exited 0: 7 library + 6 CLI + 8 integration passed (21 passed, 0 failed; 0 doc-tests), including the lock-provenance regression; command, environment, stdout, stderr, exit saved as `.pdca/open-issues/rv1/attempt-2/cargo-test.{command,environment,stdout,stderr,exit,result}`. Required `/home/alex/.cargo/bin/cargo fmt --all -- --check` final run exited 0 with empty stdout/stderr; full command/environment/output/exit under `.pdca/open-issues/rv1/attempt-2/cargo-fmt-check.*`. An initial fmt check exited 1 due formatting in the new test; `cargo fmt --all` exited 0 and the check was rerun successfully; both attempts are retained in the evidence pack. Direct `cargo run --offline --locked -- --env_info` with CMake/jobs environment exited 0 and printed Candle Roformer `1.0.0 (crates.io registry)`, candle-core `0.11.1`, CLI/package `0.1.0`, CUDA/Metal false, CPU available, parallelism 8, and both no-asset path defaults; complete command/env/stdout/stderr/exit under `.pdca/open-issues/rv1/attempt-2/env-info.*`. All runs were non-vacuous; the regression test was included in the 6-test CLI unit suite.

At this status snapshot, the implementation/evidence commit and this n2 progress update are pending; post-commit Git facts remain to be gathered and recorded. No overall CHECK verdict is recorded or inferred.

Attempt n2 implementation/evidence commit: `624d456f711e3c235a40483338df9196777f88c3` (`fix: report candle dependency provenance from lockfile`), containing `build.rs`, `src/main.rs`, this progress record and all attempt-2 test/fmt/env-info evidence. Immediately after that commit, `git rev-parse HEAD` was `624d456f711e3c235a40483338df9196777f88c3`; `git status --porcelain` exited 0 with empty stdout; `git rev-parse origin/main` remained `a1ea0a4cea0d7d19142531d91e91469e731af2e8`; branch was main ahead 13 and clean. Full exact commands/outputs and exit statuses, the 13-line `git log --oneline a1ea0a4..HEAD`, and `git diff --stat a1ea0a4..HEAD -- . ':(exclude).pdca'` are persisted in `.pdca/open-issues/rv1/attempt-2/post-commit-git-facts.md`. That diff stat is 12 non-PDCA paths, 3453 insertions and 278 deletions. The post-commit artifact and this status update were committed together in the final status follow-up. After that follow-up, `git status --porcelain` exited 0 with empty stdout and `git status` reported a clean working tree on main ahead 14; origin remained unchanged. Auditability limit: historical GitHub issue actions cannot be established from local Git history or the current-state issue snapshots; no issue action or push occurred during this DO. No verdict is declared.

## CHECK record

CHECK finding supplied for this rework (the handoff included this finding but no overall matrix/verdict; none is reconstructed):

**ENV-PROVENANCE-R1-01 — major, confirmed; AC-5:** `build.rs` runs `git rev-parse` in the CLI's `CARGO_MANIFEST_DIR` and assigns the result to `CANDLE_ROFORMER_GIT_REVISION`. The CLI prints it as the dependency's revision. The old observed output `a1ea0a4cea0d` matches exactly this mistake. The green env-info test does not check the revision's provenance.

## ACT record

`pending`

## Lessons learned

`pending: retrospective not yet conducted`

## Revision history

`none` — no plan revision has occurred.

## Defect history

upstream-v1-api-config-fields | r1/n1 | 1 | `.pdca/open-issues/rv1/attempts/cargo-test-no-run-cmake-policy-min-3.5.log` → `tests/flow.rs` fixture update → resolved by `.pdca/open-issues/rv1/attempts/cargo-test-no-run-api-fixture-fixed.log` | no recurrence; no escalation
flacenc-verify-error-display | r1/n1 | 1 | `.pdca/open-issues/rv1/attempts/cargo-test-no-run-first-implementation.log` E0277 in FLAC config formatter → changed `{error}` to `{error:?}` in `src/pipeline.rs`; no recurrence or escalation yet
env-info-temporary-path-reference | r1/n1 | 1 | `.pdca/open-issues/rv1/attempts/cargo-test-no-run-after-flac-debug-fix.log` E0515 in `src/main.rs:407` → owned model directory string; resolved by `.pdca/open-issues/rv1/attempts/cargo-test-no-run-after-env-info-fix.log`; no recurrence or escalation
custom-name-test-error-wording-expectation | r1/n1 | 1 | `.pdca/open-issues/rv1/attempts/cargo-test-ticket2-custom-names.log` assertion incorrectly required the CLI flag name in library errors → corrected test to assert reason-specific error text; resolved by `.pdca/open-issues/rv1/attempts/cargo-test-ticket2-custom-names-after-assertion-fix.log`; no recurrence
ticket2-json-test-vacuous-selection | r1/n1 | 1 | `.pdca/open-issues/rv1/attempts/cargo-test-ticket2-json-cli.log` observed 0 selected tests because `--exact` omitted the `cli_tests::` module prefix → reran qualified; `.pdca/open-issues/rv1/attempts/cargo-test-ticket2-json-cli-qualified.log` observed 1 passed; no recurrence
cli-whisper-assets-order | r1/n1 | 1 | `.pdca/open-issues/rv1/attempts/cli-ticket2-custom-sidecars.log` transcribe CLI treated model safetensors as config → reordered constructor args in `src/main.rs`; resolved by `.pdca/open-issues/rv1/attempts/cli-ticket2-custom-sidecars-after-asset-order-fix.log` (exit 0, one input, three outputs share custom vocal basename); no recurrence
ticket4-env-info-stale-binary | r1/n1 | 1 | `.pdca/open-issues/rv1/attempts/cli-ticket4-env-info-resolved-paths.log` returned exit 0 but lacked new path lines because it invoked the pre-change binary → rebuilt with `.pdca/open-issues/rv1/attempts/cargo-build-ticket4-cli.log` and reran; resolved by `.pdca/open-issues/rv1/attempts/cli-ticket4-env-info-resolved-paths-after-rebuild.log` (resolved paths shown, sentinel secret absent); no recurrence
ticket6-short-flac-cli-roundtrip | r1/n1 | 1 | `.pdca/open-issues/rv1/attempts/cargo-test-ticket6-cli-codecs.log` FLAC CLI output for 2177 resampled frames failed project `read_audio` with `unexpected end of file`; root cause was fixed-block encoder metadata using the 4096 default for a one-short-frame stream → bound FLAC frame size to actual sample count (minimum FLAC block size) in `src/pipeline.rs`; resolved by `.pdca/open-issues/rv1/attempts/cargo-test-ticket6-cli-codecs-after-blocksize-fix.log` (1/1 pass, FLAC and MP3 outputs read at 48 kHz; FLAC exact resampled sample count); no recurrence
ticket7-cli-level-test-vacuous-filter | r1/n1 | 1 | `.pdca/open-issues/rv1/attempts/cargo-test-ticket7-cli-level-flags.log` exited 0 but selected 0 cases because `--exact` omitted `cli_tests::` module prefix → reran with the fully qualified name; `.pdca/open-issues/rv1/attempts/cargo-test-ticket7-cli-level-flags-qualified.log` confirms 1 selected/passed; resolved, no recurrence
ENV-PROVENANCE-R1-01 | r1/n2 | 1 | CHECK finding supplied verbatim in `.pdca/status/open-issues-1.md` → `build.rs` reports CLI repo HEAD as registry dependency revision; removed Git lookup/revision env, now derive version/source from Cargo.lock in `build.rs` and test report against lock in `src/main.rs`; `.pdca/open-issues/rv1/attempt-2/cargo-test.stdout`, `env-info.stdout` | no recurrence observed after one fix; no escalation

## Changed files

- `.pdca/status/open-issues-1.md` → created this cycle; PLAN and original task persisted.
- `.pdca/open-issues/rv1/` → evidence root directory created.
- `.pdca/open-issues/rv1/attempts/` → attempts directory created.
- `.pdca/open-issues/rv1/attempts/cargo-test-full-final.log` → full unfiltered suite, exit 0, 21 passed/0 failed (7 library + 6 CLI + 8 integration; 0 doc-tests defined).
- `.pdca/open-issues/rv1/attempts/cargo-fmt-check-final.log` → final format check exit 0.
- `.pdca/open-issues/rv1/attempts/git-status-before-status-commit.log` → pre-status-commit full Git status, branch/HEAD, and untracked artifact inventory.
- `.pdca/status/open-issues-1.md` → final DO result, candidate AC assessment and Git-state ledger; created this cycle, first committed in `9a218e2205c762a312df00336923f69aa34190fb`, with the final ledger correction committed in this DO continuation.
- `.pdca/open-issues/rv1/attempts/` → exact full-test and prior format-check evidence; current re-anchor confirms 108 untracked paths under `.pdca/` only. This continuation will commit the cycle evidence and three superseded preparation records together without ignore rules or deletion.
- `.pdca/open-issues/rv1/attempts/git-diff-check-all-pdca.log` → staged archive whitespace check exit 2; existing logs/snapshots and the preserved superseded preparation records were not rewritten to remove whitespace.
- `.pdca/open-issues/rv1/attempts/git-status-porcelain-final.log` → exact empty stdout from post-commit `git status --porcelain` (exit 0).
- `.pdca/open-issues/rv1/attempts/git-status-final.log` → exact post-commit `git status` output (exit 0; main ahead 10; clean working tree).
- `.pdca/open-issues/rv1/attempts/git-status-final-metadata.log` → exit statuses, branch and ahead count for the two final status commands.
- `.pdca/open-issues/rv1/attempt-2/` → attempt n2/3 evidence pack for the AC-5 dependency-provenance defect; run logs and post-commit Git facts will be committed with the fix.
- `build.rs` → remove CLI repository `git rev-parse` attribution; parse unique Candle package version/source from Cargo.lock and emit truthful source kind; fail build if dependency entry is absent/ambiguous.
- `src/main.rs` → remove false Candle git revision field; report lock-derived source with version; regression test compares Candle core/Roformer versions and Roformer registry source to Cargo.lock, and checks no revision label.
- `.pdca/open-issues/rv1/attempt-2/cargo-test.*` → requested offline locked full-suite command, environment, stdout, stderr, exit and result metadata; exit 0, 21 passed, 0 failed (7+6+8; 0 doc-tests).
- `.pdca/open-issues/rv1/attempt-2/cargo-fmt-check.*` → requested final fmt check, command, environment, stdout, stderr, exit and metadata; exit 0. Initial formatting failure, formatter run, and evidence are also retained as `cargo-fmt.*` and `cargo-fmt-apply.*`.
- `.pdca/open-issues/rv1/attempt-2/env-info.*` → direct offline `--env_info` invocation and its command/environment/output/exit; exit 0, true lock provenance printed, required diagnostics retained.
- `.pdca/open-issues/rv1/attempt-2/git-diff-check.*` → staged-delta whitespace check, exit 2 due to the test runner's extra blank line at EOF in `cargo-test.stdout`; evidence was preserved unchanged.
- `.pdca/open-issues/rv1/attempt-2/post-commit-git-facts.md` → exact post-commit HEAD/origin/status/log/non-PDCA diff-stat evidence and auditability limit; captured while implementation/evidence commit `624d456f711e3c235a40483338df9196777f88c3` had a clean working tree.
- `.pdca/status/karaoke-cli-open-issues.md`, `.pdca/status/karaoke-cli-open-issues/planner-plan-r1.md`, `.pdca/status/karaoke-cli-open-issues/preparation-r1.md` → superseded, never-run orchestrator preparation records for this same task; preserved and included in the one remaining `.pdca/` commit.
- `Cargo.toml`, `Cargo.lock`, `README.md`, `src/lib.rs`, `src/main.rs`, `src/pipeline.rs`, `src/catalog.rs`, `build.rs`, `data/models.json` → initial implementation/README changes, committed in `1ed3cb908dea932196fcef99fbbbfd96135cac84`; `src/main.rs` and `tests/flow.rs` Whisper/custom-name correction and tests committed in `8920434c6afac4457006dbeeace989972384f3e5`; `src/catalog.rs` and `tests/fixtures/model-catalog.json` committed in `74b3e76146ae0e40a196ad124b10c13c7b5fb73a`; #4 diagnostics in `src/main.rs` committed in `8345e3b885e2324a431bcd61068eec0257205a1d`; #5 public `NamedStems`/`separate_audio` API in `src/lib.rs`/`src/pipeline.rs` and synthetic 1-/4-stem CLI tests in `tests/flow.rs` committed in `253e07a776e0449f12ab0923c3cf224cb18afc1e`; #6 codec support/tests and README codec section in `src/main.rs`, `src/pipeline.rs`, `tests/flow.rs`, `README.md`, plus `docs/dependency-audit.md`, committed in `1f98eb4deee4e0709e227ebe4ece376a4eeeec9f`; #7 level defaults/residual tests and complete README/dependency-audit semantics committed in `e4fe915b1c2fc1834f639974250e5f450e3ea3b4`.
- `src/catalog.rs`, `tests/fixtures/model-catalog.json` → catalogue fixture and filter/limit/pretty/JSON schema tests, committed in `74b3e76146ae0e40a196ad124b10c13c7b5fb73a`.

## Git state

Status path: `.pdca/status/open-issues-1.md` — created this cycle · tracked and updated for attempt n2/3. Baseline: branch `main`, `HEAD` `a1ea0a4cea0d7d19142531d91e91469e731af2e8`. Product commits include `1ed3cb908dea932196fcef99fbbbfd96135cac84`, `8920434c6afac4457006dbeeace989972384f3e5`, `74b3e76146ae0e40a196ad124b10c13c7b5fb73a`, `8345e3b885e2324a431bcd61068eec0257205a1d`, `253e07a776e0449f12ab0923c3cf224cb18afc1e`, `1f98eb4deee4e0709e227ebe4ece376a4eeeec9f`, and `e4fe915b1c2fc1834f639974250e5f450e3ea3b4`; evidence/status commits since n2's initial re-anchor are `ebb145264999aae589be3d6c8f79ee68b2afae90`, `a57d256da1f89e19b77e8cac73d148736f0a32c3`, `207037a7010f1e51bf582343c999193ed3ee0184`, and implementation/evidence commit `624d456f711e3c235a40483338df9196777f88c3`. Post-commit evidence after `624d456` records that exact HEAD, origin `a1ea0a4cea0d7d19142531d91e91469e731af2e8`, and clean `main` ahead 13; stored at `.pdca/open-issues/rv1/attempt-2/post-commit-git-facts.md`. The artifact and this status update are committed in the final status follow-up; post-follow-up verification observed clean working tree, `main` ahead 14, with origin unchanged. No changes outside the repository, no push, no issue actions.

## Progress log

2026-11-13T19:48:00Z | PLAN | revision r1 | attempt n1/3 | plan ready; status initialized; evidence root and attempts directory created | `.pdca/status/open-issues-1.md`; `.pdca/open-issues/rv1/`
2026-11-13T19:54:43Z | DO | revision r1 | attempt n1/3 | DO started; frozen plan hash and planned source anchors verified; U1 running | `.pdca/status/open-issues-1.md`; `.pdca/open-issues/rv1/`
2026-11-13T19:56:00Z | DO | revision r1 | attempt n1/3 | initial cargo update exited 127 before cargo launch; locating tool via explicit path, no fix count | `.pdca/open-issues/rv1/attempts/cargo-update-candle-roformer.log`
2026-11-13T19:58:14Z | DO | revision r1 | attempt n1/3 | cargo update to candle-roformer 1.0.0 exit 0; cargo fetch exit 0; public API inspected and evidence snapshot persisted | `.pdca/open-issues/rv1/attempts/cargo-update-candle-roformer-1.0.0.log`; `.pdca/open-issues/rv1/attempts/cargo-fetch-1.0.0.log`; `.pdca/open-issues/rv1/source/candle-roformer-1.0.0/`
2026-11-13T19:59:21Z | DO | revision r1 | attempt n1/3 | codec crate search exit 0 for flacenc and mp3lame-encoder; initial compile wrapper exited 127 before cargo launch | `.pdca/open-issues/rv1/attempts/cargo-search-flacenc.log`; `.pdca/open-issues/rv1/attempts/cargo-search-mp3lame.log`; `.pdca/open-issues/rv1/attempts/cargo-test-no-run-upstream-v1.0.log`
2026-11-13T20:05:51Z | DO | revision r1 | attempt n1/3 | CMake compatibility env unblocked upstream C build; no-run compile then found 1 API compile error in tiny test fixture (missing new optional config fields) | `.pdca/open-issues/rv1/attempts/cargo-test-no-run-cmake-policy-min-3.5.log`
2026-11-13T20:03:33Z | DO | revision r1 | attempt n1/3 | corrected test compile exited 101 at libsamplerate-sys bundled CMake configure; trying advertised policy compatibility setting, no product defect fix count | `.pdca/open-issues/rv1/attempts/cargo-test-no-run-upstream-v1.0-corrected.log`
2026-11-13T20:08:24Z | DO | revision r1 | attempt n1/3 | CMake policy compatibility compile succeeded after adding the two new upstream config fields to the synthetic fixture; 3 test binaries built; codec crate metadata inspected | `.pdca/open-issues/rv1/attempts/cargo-test-no-run-api-fixture-fixed.log`; `.pdca/open-issues/rv1/attempts/cargo-info-flacenc.log`; `.pdca/open-issues/rv1/attempts/cargo-info-mp3lame-encoder.log`
2026-11-13T20:09:57Z | DO | revision r1 | attempt n1/3 | bookkeeping correction: moved API-compatibility defect to Defect history, corrected CHECK-not-run reason and refreshed same-cycle Git ledger; prior progress rows stay append-only despite a late-written 20:03 entry | `.pdca/status/open-issues-1.md`
2026-11-13T20:14:02Z | DO | revision r1 | attempt n1/3 | mp3lame-sys source confirms bundled LAME statically builds with available cc, no system package needed; codec source snapshots and hashes persisted | `.pdca/open-issues/rv1/attempts/cargo-info-mp3lame-sys.log`; `.pdca/open-issues/rv1/source/flacenc-0.5.1/`; `.pdca/open-issues/rv1/source/mp3lame-encoder-0.3.0/`; `.pdca/open-issues/rv1/source/mp3lame-sys-0.2.0/`
2026-11-13T20:21:47Z | DO | revision r1 | attempt n1/3 | Recovered after compaction; re-anchored Cargo.toml/Cargo.lock, src/lib.rs, src/main.rs, src/pipeline.rs, tests/flow.rs and README.md at the planned identities; no semantic drift and no moved anchors; U1 closed on compile evidence, U2 started | `.pdca/status/open-issues-1.md`; `.pdca/open-issues/rv1/attempts/cargo-test-no-run-api-fixture-fixed.log`
2026-11-13T20:39:14Z | DO | revision r1 | attempt n1/3 | Same-attempt continuation; build failed with E0277 in FLAC config error formatter, fix applied with Debug formatting and build rerun pending | `.pdca/open-issues/rv1/attempts/cargo-test-no-run-first-implementation.log`; `src/pipeline.rs`
2026-11-13T20:40:53Z | DO | revision r1 | attempt n1/3 | FLAC Debug-format correction compiled past E0277; no-run remains exit 101 on E0515 temporary environment string borrow in env_info; fix pending | `.pdca/open-issues/rv1/attempts/cargo-test-no-run-after-flac-debug-fix.log`; `src/main.rs:407`
2026-11-13T20:43:11Z | DO | revision r1 | attempt n1/3 | E0515 fixed; `cargo test --no-run` exit 0, exactly 3 test executables built; compile gate green | `.pdca/open-issues/rv1/attempts/cargo-test-no-run-after-env-info-fix.log`
2026-11-13T20:46:20Z | DO | revision r1 | attempt n1/3 | Ticket #1 implementation now routes the CLI through a continue-on-error batch runner; added an integration fixture for sorted directory expansion plus valid/corrupt mixed inputs; targeted test pending | `src/pipeline.rs`; `src/main.rs`; `tests/flow.rs`
2026-11-13T20:49:34Z | DO | revision r1 | attempt n1/3 | Ticket #1 integration test exited 0 (1 selected, 1 passed); expected-negative CLI batch run exited 1 for declared corrupt `b-damaged.wav`, while valid `a-valid.wav` produced 2 stems; CLI summary reported 2 inputs, 1 success, 1 failure | `.pdca/open-issues/rv1/attempts/cargo-test-ticket1-batch.log`; `.pdca/open-issues/rv1/attempts/cli-ticket1-mixed-batch.log`
2026-11-13T20:52:35Z | DO | revision r1 | attempt n1/3 | Ticket #1 AC-2 closed on targeted evidence: directory order, per-file preflight/no partial file for rejected a-valid, continue after damaged b-valid, later c-valid output, summary, nonzero exit; CLI expected-negative exit 1 for 3 selected, 1 succeeded, 2 failed; commit pending | `.pdca/open-issues/rv1/attempts/cargo-test-ticket1-preflight-continue.log`; `.pdca/open-issues/rv1/attempts/cli-ticket1-per-file-preflight.log`
2026-11-13T20:55:27Z | DO | revision r1 | attempt n1/3 | Product implementation including #1 and candidate #2–#8 plus initial README edits committed locally as `1ed3cb908dea932196fcef99fbbbfd96135cac84`; `git status` shows no product modifications, only untracked `.pdca/` cycle records/evidence; continue acceptance tests in ticket order | `git show --stat HEAD`; `git status --short --branch`
2026-11-13T21:03:27Z | DO | revision r1 | attempt n1/3 | U3 custom-name flow test first failed on an over-strict error-wording assertion, then passed after narrowing expected text (1 passed); CLI parser test first selected 0 cases (vacuous failure), then qualified rerun passed 1; CLI custom-name run exited 0 and emitted custom Vocals plus default Instrumental; transcription CLI attempt expected-negative exit 1 exposed Whisper constructor path ordering defect; correction pending | `.pdca/open-issues/rv1/attempts/cargo-test-ticket2-custom-names.log`; `.pdca/open-issues/rv1/attempts/cargo-test-ticket2-custom-names-after-assertion-fix.log`; `.pdca/open-issues/rv1/attempts/cargo-test-ticket2-json-cli.log`; `.pdca/open-issues/rv1/attempts/cargo-test-ticket2-json-cli-qualified.log`; `.pdca/open-issues/rv1/attempts/cli-ticket2-custom-name.log`; `.pdca/open-issues/rv1/attempts/cli-ticket2-custom-sidecars.log`
2026-11-13T21:05:38Z | DO | revision r1 | attempt n1/3 | CLI sidecar evidence after constructor fix exit 0: 1 input succeeded, outputs `cli-transcript-name.wav`, `.txt`, `.json`, plus default Instrumental; committed unit #2 as `8920434c6afac4457006dbeeace989972384f3e5`; current unit #3 started | `.pdca/open-issues/rv1/attempts/cli-ticket2-custom-sidecars-after-asset-order-fix.log`; `git show --stat HEAD`
2026-11-13T21:09:28Z | DO | revision r1 | attempt n1/3 | Unit #3 verified offline: fixture test 1/1 passed; CLI JSON filter+limit selected 1 of 3; built-in pretty listed 3; committed as `74b3e76146ae0e40a196ad124b10c13c7b5fb73a`; stopped before starting #4 | `.pdca/open-issues/rv1/attempts/cargo-test-ticket3-catalog-fixture.log`; `.pdca/open-issues/rv1/attempts/cli-ticket3-manifest-filter-limit-json.log`; `.pdca/open-issues/rv1/attempts/cli-ticket3-builtin-pretty.log`
2026-11-13T21:13:51Z | DO | revision r1 | attempt n1/3 | Re-anchored committed #1–#3 state; #4 DO started. Inspected published candle-roformer 1.0.0 `src/lib.rs` exports and `separator.rs`: `Stems` has named `stems: Vec<(String, AudioBuffer)>`, `get/names/iter`, convenience `vocals/instrumental`; `num_stems == 1` means residual, `>1` means independent estimates; 4 stems default to drums/bass/other/vocals | `.pdca/open-issues/rv1/source/candle-roformer-1.0.0/src/lib.rs`; `.pdca/open-issues/rv1/source/candle-roformer-1.0.0/src/separator.rs`; `.pdca/open-issues/rv1/source/candle-roformer-1.0.0/src/config.rs`
2026-11-13T21:17:28Z | DO | revision r1 | attempt n1/3 | #4 CLI unit suite passed 4/4; first env_info subprocess runs exited 0, but the resolved-path run used a stale normal executable built before the code edit and omitted the new path lines; rebuilding the executable and rerunning CLI checks pending | `.pdca/open-issues/rv1/attempts/cargo-test-ticket4-cli-unit.log`; `.pdca/open-issues/rv1/attempts/cli-ticket4-env-info-no-weights.log`; `.pdca/open-issues/rv1/attempts/cli-ticket4-env-info-resolved-paths.log`
2026-11-13T21:20:01Z | DO | revision r1 | attempt n1/3 | Rebuilt binary offline; env_info CLI confirmed versions/features/CPU/parallelism, resolved checkpoint+config paths with nonexistent weights, and no secret sentinel; default output had zero timing lines; `--debug` emitted all four timestamped module-target stage timings; corrupt input expected-negative exit 1 included path and cause; committed #4 as `8345e3b885e2324a431bcd61068eec0257205a1d`; #5 starts now | `.pdca/open-issues/rv1/attempts/cargo-build-ticket4-cli.log`; `.pdca/open-issues/rv1/attempts/cli-ticket4-env-info-no-weights-after-rebuild.log`; `.pdca/open-issues/rv1/attempts/cli-ticket4-env-info-resolved-paths-after-rebuild.log`; `.pdca/open-issues/rv1/attempts/cli-ticket4-default-output.log`; `.pdca/open-issues/rv1/attempts/cli-ticket4-debug-stage-timings.log`; `.pdca/open-issues/rv1/attempts/cli-ticket4-error-path-cause.log`
2026-11-13T21:25:48Z | DO | revision r1 | attempt n1/3 | #5 synthetic integration test exit 0 (1 passed): actual 1.0 4-stem profile returned all 4 independent named buffers and CLI wrote all 4 filenames; `--single_stem bass` wrote exactly one; 1-stem wrapper returned vocals/instrumental with exact residual reconstruction (<1e-4); commit `253e07a776e0449f12ab0923c3cf224cb18afc1e`; stopped before unit #6 | `.pdca/open-issues/rv1/attempts/cargo-test-ticket5-four-stem-and-residual.log`; `.pdca/open-issues/rv1/source/candle-roformer-1.0.0/src/separator.rs`; `git show --stat HEAD`
2026-11-13T21:30:47Z | DO | revision r1 | attempt n1/3 | Same-attempt continuation re-anchored at #5 commit; source anchors intact with no semantic/cosmetic drift; product tree clean; inspected #6/#7 acceptance details and upstream 1.0 level/residual semantics; ticket #6 started, #7 and docs/full-suite pending | `.pdca/status/open-issues-1.md`; `.pdca/open-issues/rv1/source/candle-roformer-1.0.0/src/separator.rs`; `.pdca/open-issues/rv1/source/candle-roformer-1.0.0/src/dsp.rs`
2026-11-13T21:39:58Z | DO | revision r1 | attempt n1/3 | Ticket #6 targeted codec tests passed (5/5) and CLI flag test passed (1/1); extended CLI integration found a failure: 2000-frame synthetic FLAC encoded and was recognized as FLAC/48 kHz/2177 samples, but project `read_audio` returned `unexpected end of file`; failure recorded, investigation/fix required before #6 can close | `.pdca/open-issues/rv1/attempts/cargo-test-ticket6-output-tests.log`; `.pdca/open-issues/rv1/attempts/cargo-test-ticket6-cli-flags.log`; `.pdca/open-issues/rv1/attempts/cargo-test-ticket6-cli-codecs.log`; `.pdca/open-issues/rv1/attempts/cli-ticket6-flac-file-identification.log`
2026-11-13T21:45:55Z | DO | revision r1 | attempt n1/3 | Fixed short FLAC stream block-size metadata: encode using the actual (bounded) frame count; CLI integration rerun passed 1/1 for FLAC and MP3 at 48 kHz, and the FLAC decoder returned the exact resampled sample count. Python lockfile-delta helper was unavailable because this environment's Python lacks `tomllib` (exit 1); direct package versions were read from Cargo.lock with grep (exit 0) and documentation records Cargo.lock as authoritative. #6 acceptance tests/docs ready for commit | `src/pipeline.rs`; `.pdca/open-issues/rv1/attempts/cargo-test-ticket6-cli-codecs-after-blocksize-fix.log`; `.pdca/open-issues/rv1/attempts/dependency-lock-delta.log`; `.pdca/open-issues/rv1/attempts/dependency-direct-versions.log`
2026-11-13T21:48:28Z | DO | revision r1 | attempt n1/3 | #6 completed and committed as `1f98eb4deee4e0709e227ebe4ece376a4eeeec9f`; post-fix codec unit suite 5/5, CLI codec flag parser 1/1, and synthetic CLI FLAC/MP3 at 48 kHz integration 1/1; WAV f32 values exact, FLAC exact sample count/rate, MP3 duration ≤0.05 s; started #7 | `.pdca/open-issues/rv1/attempts/cargo-test-ticket6-output-tests-after-blocksize-fix.log`; `.pdca/open-issues/rv1/attempts/cargo-test-ticket6-cli-flags.log`; `.pdca/open-issues/rv1/attempts/cargo-test-ticket6-cli-codecs-after-blocksize-fix.log`; `.pdca/open-issues/rv1/attempts/git-commit-ticket6.log`
2026-11-13T21:53:34Z | DO | revision r1 | attempt n1/3 | #7 synthetic level integration passed 1/1, proving ≤1e-5 exact residual, normalized .9 output and input scale-invariant behavior, amplification residual against its .1 reference, and secondary stem matching spectral inversion. CLI level parser command exited 0 but selected 0 tests because exact filter omitted module prefix; recorded as vacuous failure and qualified rerun pending | `.pdca/open-issues/rv1/attempts/cargo-test-ticket7-level-semantics.log`; `.pdca/open-issues/rv1/attempts/cargo-test-ticket7-cli-level-flags.log`
2026-11-13T21:57:56Z | DO | revision r1 | attempt n1/3 | Re-anchored at committed #6 HEAD; verified the #7 qualified CLI rerun is 1/1 and corrected the durable vacuous-test defect history; current #7 code/docs remain uncommitted as user stated; final README transcription/test prose, dependency-audit #7 note, full suite and format verification pending | `.pdca/status/open-issues-1.md`; `.pdca/open-issues/rv1/attempts/cargo-test-ticket7-cli-level-flags-qualified.log`; `.pdca/open-issues/rv1/attempts/cargo-test-ticket7-level-semantics.log`
2026-11-13T22:02:26Z | DO | revision r1 | attempt n1/3 | #7 documentation corrected for actual transcribe/output staging behavior and test scope; dependency audit records that no new level-processing dependency was added; #7 committed as `e4fe915b1c2fc1834f639974250e5f450e3ea3b4`; targeted tests are 1/1 integration and 1/1 qualified CLI; full cargo test and final formatting/status check pending | `.pdca/open-issues/rv1/attempts/git-commit-ticket7.log`; `.pdca/open-issues/rv1/attempts/cargo-test-ticket7-level-semantics.log`; `.pdca/open-issues/rv1/attempts/cargo-test-ticket7-cli-level-flags-qualified.log`
2026-11-13T22:08:23Z | DO | revision r1 | attempt n1/3 | Full unfiltered suite exit 0: 21 passed, 0 failed (7 library, 6 CLI, 8 integration; doc-tests 0); final cargo fmt check exit 0. AC-1–AC-8 candidate met; AC-9 blocked by persistent evidence and three unrelated untracked status files, so the final worktree cannot be made clean without unauthorized staging/deletion. Final status commit pending; CHECK not run; candidate routes to ACT incomplete. | `.pdca/open-issues/rv1/attempts/cargo-test-full-final.log`; `.pdca/open-issues/rv1/attempts/cargo-fmt-check-final.log`; `.pdca/open-issues/rv1/attempts/git-status-before-status-commit.log`
2026-11-13T22:12:07Z | DO | revision r1 | attempt n1/3 | First final-status commit `9a218e2205c762a312df00336923f69aa34190fb` succeeded; post-commit inspection observed branch ahead 8 with persistent untracked evidence and three unrelated status artifacts, confirming AC-9 remains blocked. CHECK is not run; status ledger correction records the DO → ACT handoff and is committed in this continuation. | `.pdca/open-issues/rv1/attempts/git-commit-cycle-status.log`; `.pdca/open-issues/rv1/attempts/cargo-test-full-final.log`; `.pdca/open-issues/rv1/attempts/cargo-fmt-check-final.log`
2026-11-13T22:21:26Z | DO | revision r1 | attempt n1/3 | Recovered/re-anchored at `92150b8bbe8b0aee77a1e68766a2c0b3e036290e`; verified 108 untracked paths, all under `.pdca/`, three preserved preparation records, tracked cycle status, and branch ahead 9. User authorized one commit of all remaining `.pdca/`; no `.gitignore` edit or deletion; full suite evidence is 21 passed, 0 failed. | `.pdca/open-issues/rv1/attempts/cargo-test-full-final.log`; `.pdca/status/open-issues-1.md`
2026-11-13T22:24:15Z | DO | revision r1 | attempt n1/3 | Staged all remaining `.pdca/` paths (109 staged paths including tracked status update); `git diff --cached --check` exited 2 on whitespace in preserved logs/snapshots and superseded preparation records; no whitespace cleanup or record rewrite performed. One-commit staging retained. | `.pdca/open-issues/rv1/attempts/git-diff-check-all-pdca.log`
2026-11-13T22:27:08Z | DO | revision r1 | attempt n1/3 | Single commit staged 110 `.pdca/` paths; post-commit status was clean at branch ahead 10, and exact porcelain/status outputs were persisted. Final DO status now records AC-9 closure, exact suite counts, preservation/no-ignore rationale, and proof paths; proof logs and status closure are folded into the same local commit. No CHECK run. | `.pdca/open-issues/rv1/attempts/git-status-porcelain-final.log`; `.pdca/open-issues/rv1/attempts/git-status-final.log`; `.pdca/open-issues/rv1/attempts/git-status-final-metadata.log`; `.pdca/open-issues/rv1/attempts/cargo-test-full-final.log`
2026-11-13T22:51:05Z | DO | revision r1 | attempt n2/3 | CHECK→DO started for confirmed AC-5 defect `ENV-PROVENANCE-R1-01`; re-anchor found HEAD `ebb145264999aae589be3d6c8f79ee68b2afae90`, origin `a1ea0a4cea0d7d19142531d91e91469e731af2e8`, main ahead 10 and clean. Status records the finding verbatim; source edit pending. | `.pdca/status/open-issues-1.md`; `build.rs`; `src/main.rs`; `Cargo.lock`
2026-11-13T22:59:56Z | DO | revision r1 | attempt n2/3 | Notice: re-anchor after n2 start found HEAD moved `ebb145264999aae589be3d6c8f79ee68b2afae90` → `207037a7010f1e51bf582343c999193ed3ee0184` and `main` ahead 10 → 12 via commits `a57d256da1f89e19b77e8cac73d148736f0a32c3` and `207037a7010f1e51bf582343c999193ed3ee0184`; `git diff ebb1452..HEAD` is confined to `.pdca/open-issues/rv1/check-r1/**`, no product anchor changed. Current status-file modification is cycle bookkeeping; source files remain unchanged before this fix. | `.pdca/open-issues/rv1/check-r1/`; `.pdca/status/open-issues-1.md`
2026-11-13T23:05:51Z | DO | revision r1 | attempt n2/3 | Applied one fix for `ENV-PROVENANCE-R1-01`: removed dependency revision sourced from repo HEAD, emit Cargo.lock version/source, and added lock-backed env-info regression coverage. Full suite exit 0 (7 library + 6 CLI + 8 integration; 21 passed, 0 failed; no doc tests); final fmt check exit 0; direct --env_info exit 0 and prints 1.0.0 (crates.io registry). Initial fmt check exit 1 was corrected with rustfmt and rerun. | `.pdca/open-issues/rv1/attempt-2/cargo-test.result`; `.pdca/open-issues/rv1/attempt-2/cargo-fmt-check.result`; `.pdca/open-issues/rv1/attempt-2/env-info.result`
2026-11-13T23:13:45Z | DO | revision r1 | attempt n2/3 | Implementation/evidence commit `624d456f711e3c235a40483338df9196777f88c3` created. Post-commit capture: HEAD exact, origin/main unchanged at a1ea0a4, porcelain empty, branch main ahead 13 and clean; log and non-PDCA diff-stat captured; historical issue-action audit limit recorded. Final status update and clean-tree confirmation pending. | `.pdca/open-issues/rv1/attempt-2/post-commit-git-facts.md`
