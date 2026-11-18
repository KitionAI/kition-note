//! CodeMirror 5's simple modes (rule tables of regular expressions, a state each), run as the
//! mode's runner runs them: in the current state, the first rule whose pattern matches where the
//! stream stands takes the text, styles it, and may move to another state; where none does, a
//! byte goes unstyled. Ported this way: NSIS and WebAssembly text (see each table).

use std::sync::OnceLock;

use super::Kind;
use super::regex::Regex;
use super::scan::{self, Out};

/// One rule: its pattern, its style, and the state it moves to.
pub struct Rule {
    pattern: &'static str,
    ignore_case: bool,
    token: Option<Kind>,
    next: Option<&'static str>,
}

/// A table compiled.
type Compiled = Vec<(&'static str, Vec<(Regex, Option<Kind>, Option<&'static str>)>)>;

fn compile(table: &'static [(&'static str, &'static [Rule])]) -> Compiled {
    table
        .iter()
        .map(|(state, rules)| (*state, rules.iter().map(|rule| (Regex::new(rule.pattern, rule.ignore_case), rule.token, rule.next)).collect()))
        .collect()
}

/// Which table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    Nsis,
    Wast,
}

fn compiled(table: Table) -> &'static Compiled {
    static NSIS_COMPILED: OnceLock<Compiled> = OnceLock::new();
    static WAST_COMPILED: OnceLock<Compiled> = OnceLock::new();
    match table {
        Table::Nsis => NSIS_COMPILED.get_or_init(|| compile(NSIS)),
        Table::Wast => WAST_COMPILED.get_or_init(|| compile(WAST)),
    }
}

/// The state a simple mode is in (`None`: its start).
#[derive(Clone, Debug, Default)]
pub struct SimpleMode {
    state: Option<&'static str>,
}

pub fn line(text: &str, state: &mut SimpleMode, table: Table, out: &mut Out) {
    let rules = compiled(table);
    scan::run_again(text, out, |stream| {
        let current = state.state.unwrap_or("start");
        let Some((_, list)) = rules.iter().find(|(name, _)| *name == current) else {
            stream.next();
            return None;
        };
        for (regex, token, next) in list {
            if let Some((end, _)) = regex.match_at(stream.bytes, stream.pos) {
                stream.pos = end;
                if let Some(next) = next {
                    state.state = Some(next);
                }
                return *token;
            }
        }
        stream.next();
        None
    });
}

/// The nsis mode's rules, state by state (each pattern as the mode writes it).
pub const NSIS: &[(&str, &[Rule])] = &[
    (
        "start",
        &[
            Rule {
                pattern: "(?:[+-]?)(?:0x[\\d,a-f]+)|(?:0o[0-7]+)|(?:0b[0,1]+)|(?:\\d+.?\\d*)",
                ignore_case: false,
                token: Some(Kind::Number),
                next: None,
            },
            Rule { pattern: "\"(?:[^\\\\\"]|\\\\.)*\"?", ignore_case: false, token: Some(Kind::String), next: None },
            Rule { pattern: "'(?:[^\\\\']|\\\\.)*'?", ignore_case: false, token: Some(Kind::String), next: None },
            Rule { pattern: "`(?:[^\\\\`]|\\\\.)*`?", ignore_case: false, token: Some(Kind::String), next: None },
            Rule {
                pattern: "^\\s*(?:\\!(addincludedir|addplugindir|appendfile|cd|define|delfile|echo|error|execute|finalize|getdllversion|gettlbversion|include|insertmacro|macro|macroend|makensis|packhdr|pragma|searchparse|searchreplace|system|tempfile|undef|uninstfinalize|verbose|warning))\\b",
                ignore_case: false,
                token: Some(Kind::Keyword),
                next: None,
            },
            Rule { pattern: "^\\s*(?:\\!(if(?:n?def)?|ifmacron?def|macro))\\b", ignore_case: false, token: Some(Kind::Keyword), next: None },
            Rule { pattern: "^\\s*(?:\\!(else|endif|macroend))\\b", ignore_case: false, token: Some(Kind::Keyword), next: None },
            Rule {
                pattern: "^\\s*(?:Abort|AddBrandingImage|AddSize|AllowRootDirInstall|AllowSkipFiles|AutoCloseWindow|BGFont|BGGradient|BrandingText|BringToFront|Call|CallInstDLL|Caption|ChangeUI|CheckBitmap|ClearErrors|CompletedText|ComponentText|CopyFiles|CRCCheck|CreateDirectory|CreateFont|CreateShortCut|Delete|DeleteINISec|DeleteINIStr|DeleteRegKey|DeleteRegValue|DetailPrint|DetailsButtonText|DirText|DirVar|DirVerify|EnableWindow|EnumRegKey|EnumRegValue|Exch|Exec|ExecShell|ExecShellWait|ExecWait|ExpandEnvStrings|File|FileBufSize|FileClose|FileErrorText|FileOpen|FileRead|FileReadByte|FileReadUTF16LE|FileReadWord|FileWriteUTF16LE|FileSeek|FileWrite|FileWriteByte|FileWriteWord|FindClose|FindFirst|FindNext|FindWindow|FlushINI|GetCurInstType|GetCurrentAddress|GetDlgItem|GetDLLVersion|GetDLLVersionLocal|GetErrorLevel|GetFileTime|GetFileTimeLocal|GetFullPathName|GetFunctionAddress|GetInstDirError|GetKnownFolderPath|GetLabelAddress|GetTempFileName|GetWinVer|Goto|HideWindow|Icon|IfAbort|IfErrors|IfFileExists|IfRebootFlag|IfRtlLanguage|IfShellVarContextAll|IfSilent|InitPluginsDir|InstallButtonText|InstallColors|InstallDir|InstallDirRegKey|InstProgressFlags|InstType|InstTypeGetText|InstTypeSetText|Int64Cmp|Int64CmpU|Int64Fmt|IntCmp|IntCmpU|IntFmt|IntOp|IntPtrCmp|IntPtrCmpU|IntPtrOp|IsWindow|LangString|LicenseBkColor|LicenseData|LicenseForceSelection|LicenseLangString|LicenseText|LoadAndSetImage|LoadLanguageFile|LockWindow|LogSet|LogText|ManifestDPIAware|ManifestLongPathAware|ManifestMaxVersionTested|ManifestSupportedOS|MessageBox|MiscButtonText|Name|Nop|OutFile|Page|PageCallbacks|PEAddResource|PEDllCharacteristics|PERemoveResource|PESubsysVer|Pop|Push|Quit|ReadEnvStr|ReadINIStr|ReadRegDWORD|ReadRegStr|Reboot|RegDLL|Rename|RequestExecutionLevel|ReserveFile|Return|RMDir|SearchPath|SectionGetFlags|SectionGetInstTypes|SectionGetSize|SectionGetText|SectionIn|SectionSetFlags|SectionSetInstTypes|SectionSetSize|SectionSetText|SendMessage|SetAutoClose|SetBrandingImage|SetCompress|SetCompressor|SetCompressorDictSize|SetCtlColors|SetCurInstType|SetDatablockOptimize|SetDateSave|SetDetailsPrint|SetDetailsView|SetErrorLevel|SetErrors|SetFileAttributes|SetFont|SetOutPath|SetOverwrite|SetRebootFlag|SetRegView|SetShellVarContext|SetSilent|ShowInstDetails|ShowUninstDetails|ShowWindow|SilentInstall|SilentUnInstall|Sleep|SpaceTexts|StrCmp|StrCmpS|StrCpy|StrLen|SubCaption|Unicode|UninstallButtonText|UninstallCaption|UninstallIcon|UninstallSubCaption|UninstallText|UninstPage|UnRegDLL|Var|VIAddVersionKey|VIFileVersion|VIProductVersion|WindowIcon|WriteINIStr|WriteRegBin|WriteRegDWORD|WriteRegExpandStr|WriteRegMultiStr|WriteRegNone|WriteRegStr|WriteUninstaller|XPStyle)\\b",
                ignore_case: false,
                token: Some(Kind::Keyword),
                next: None,
            },
            Rule { pattern: "^\\s*(?:Function|PageEx|Section(?:Group)?)\\b", ignore_case: false, token: Some(Kind::Keyword), next: None },
            Rule { pattern: "^\\s*(?:(Function|PageEx|Section(?:Group)?)End)\\b", ignore_case: false, token: Some(Kind::Keyword), next: None },
            Rule {
                pattern: "\\b(?:ARCHIVE|FILE_ATTRIBUTE_ARCHIVE|FILE_ATTRIBUTE_HIDDEN|FILE_ATTRIBUTE_NORMAL|FILE_ATTRIBUTE_OFFLINE|FILE_ATTRIBUTE_READONLY|FILE_ATTRIBUTE_SYSTEM|FILE_ATTRIBUTE_TEMPORARY|HIDDEN|HKCC|HKCR(32|64)?|HKCU(32|64)?|HKDD|HKEY_CLASSES_ROOT|HKEY_CURRENT_CONFIG|HKEY_CURRENT_USER|HKEY_DYN_DATA|HKEY_LOCAL_MACHINE|HKEY_PERFORMANCE_DATA|HKEY_USERS|HKLM(32|64)?|HKPD|HKU|IDABORT|IDCANCEL|IDD_DIR|IDD_INST|IDD_INSTFILES|IDD_LICENSE|IDD_SELCOM|IDD_UNINST|IDD_VERIFY|IDIGNORE|IDNO|IDOK|IDRETRY|IDYES|MB_ABORTRETRYIGNORE|MB_DEFBUTTON1|MB_DEFBUTTON2|MB_DEFBUTTON3|MB_DEFBUTTON4|MB_ICONEXCLAMATION|MB_ICONINFORMATION|MB_ICONQUESTION|MB_ICONSTOP|MB_OK|MB_OKCANCEL|MB_RETRYCANCEL|MB_RIGHT|MB_RTLREADING|MB_SETFOREGROUND|MB_TOPMOST|MB_USERICON|MB_YESNO|MB_YESNOCANCEL|NORMAL|OFFLINE|READONLY|SHCTX|SHELL_CONTEXT|SW_HIDE|SW_SHOWDEFAULT|SW_SHOWMAXIMIZED|SW_SHOWMINIMIZED|SW_SHOWNORMAL|SYSTEM|TEMPORARY)\\b",
                ignore_case: false,
                token: None,
                next: None,
            },
            Rule {
                pattern: "\\b(?:admin|all|auto|both|bottom|bzip2|components|current|custom|directory|false|force|hide|highest|ifdiff|ifnewer|instfiles|lastused|leave|left|license|listonly|lzma|nevershow|none|normal|notset|off|on|right|show|silent|silentlog|textonly|top|true|try|un\\.components|un\\.custom|un\\.directory|un\\.instfiles|un\\.license|uninstConfirm|user|Win10|Win7|Win8|WinVista|zlib)\\b",
                ignore_case: false,
                token: Some(Kind::Property),
                next: None,
            },
            Rule {
                pattern: "\\$\\{(?:And(?:If(?:Not)?|Unless)|Break|Case(?:Else)?|Continue|Default|Do(?:Until|While)?|Else(?:If(?:Not)?|Unless)?|End(?:If|Select|Switch)|Exit(?:Do|For|While)|For(?:Each)?|If(?:Cmd|Not(?:Then)?|Then)?|Loop(?:Until|While)?|Or(?:If(?:Not)?|Unless)|Select|Switch|Unless|While)\\}",
                ignore_case: false,
                token: Some(Kind::Variable),
                next: None,
            },
            Rule {
                pattern: "\\$\\{(?:BannerTrimPath|DirState|DriveSpace|Get(BaseName|Drives|ExeName|ExePath|FileAttributes|FileExt|FileName|FileVersion|Options|OptionsS|Parameters|Parent|Root|Size|Time)|Locate|RefreshShellIcons)\\}",
                ignore_case: false,
                token: Some(Kind::Variable),
                next: None,
            },
            Rule {
                pattern: "\\$\\{(?:Memento(?:Section(?:Done|End|Restore|Save)?|UnselectedSection))\\}",
                ignore_case: false,
                token: Some(Kind::Variable),
                next: None,
            },
            Rule {
                pattern: "\\$\\{(?:Config(?:Read|ReadS|Write|WriteS)|File(?:Join|ReadFromEnd|Recode)|Line(?:Find|Read|Sum)|Text(?:Compare|CompareS)|TrimNewLines)\\}",
                ignore_case: false,
                token: Some(Kind::Variable),
                next: None,
            },
            Rule {
                pattern: "\\$\\{(?:(?:At(?:Least|Most)|Is)(?:ServicePack|Win(?:7|8|10|95|98|200(?:0|3|8(?:R2)?)|ME|NT4|Vista|XP))|Is(?:NT|Server))\\}",
                ignore_case: false,
                token: Some(Kind::Variable),
                next: None,
            },
            Rule {
                pattern: "\\$\\{(?:StrFilterS?|Version(?:Compare|Convert)|Word(?:AddS?|Find(?:(?:2|3)X)?S?|InsertS?|ReplaceS?))\\}",
                ignore_case: false,
                token: Some(Kind::Variable),
                next: None,
            },
            Rule { pattern: "\\$\\{(?:RunningX64)\\}", ignore_case: false, token: Some(Kind::Variable), next: None },
            Rule { pattern: "\\$\\{(?:Disable|Enable)X64FSRedirection\\}", ignore_case: false, token: Some(Kind::Variable), next: None },
            Rule { pattern: "(#|;).*", ignore_case: false, token: Some(Kind::Comment), next: None },
            Rule { pattern: "\\/\\*", ignore_case: false, token: Some(Kind::Comment), next: Some("comment") },
            Rule { pattern: "[-+\\/*=<>!]+", ignore_case: false, token: Some(Kind::Operator), next: None },
            Rule { pattern: "\\$\\w+", ignore_case: false, token: Some(Kind::Variable), next: None },
            Rule { pattern: "\\${[\\w\\.:-]+}", ignore_case: false, token: Some(Kind::Variable), next: None },
            Rule { pattern: "\\$\\([\\w\\.:-]+\\)", ignore_case: false, token: Some(Kind::Variable), next: None },
        ],
    ),
    (
        "comment",
        &[
            Rule { pattern: ".*?\\*\\/", ignore_case: false, token: Some(Kind::Comment), next: Some("start") },
            Rule { pattern: ".*", ignore_case: false, token: Some(Kind::Comment), next: None },
        ],
    ),
];

/// The wast mode's rules, state by state (each pattern as the mode writes it).
pub const WAST: &[(&str, &[Rule])] = &[
    (
        "start",
        &[
            Rule {
                pattern: "[+\\-]?(?:nan(?::0x[0-9a-fA-F]+)?|infinity|inf|0x[0-9a-fA-F]+\\.?[0-9a-fA-F]*p[+\\/-]?\\d+|\\d+(?:\\.\\d*)?[eE][+\\-]?\\d*|\\d+\\.\\d*|0x[0-9a-fA-F]+|\\d+)",
                ignore_case: false,
                token: Some(Kind::Number),
                next: None,
            },
            Rule {
                pattern: "align|block|br(_if|_table|_on_(cast|data|func|i31|null))?|call(_indirect|_ref)?|current_memory|\\bdata\\b|catch(_all)?|delegate|drop|elem|else|end|export|\\bextern\\b|\\bfunc\\b|global(\\.(get|set))?|if|import|local(\\.(get|set|tee))?|loop|module|mut|nop|offset|param|result|rethrow|return(_call(_indirect|_ref)?)?|select|start|table(\\.(size|get|set|size|grow|fill|init|copy))?|then|throw|try|type|unreachable|unwind|i(32|64)\\.(store(8|16)|(load(8|16)_[su]))|i64\\.(load32_[su]|store32)|[fi](32|64)\\.(const|load|store)|f(32|64)\\.(abs|add|ceil|copysign|div|eq|floor|[gl][et]|max|min|mul|nearest|neg?|sqrt|sub|trunc)|i(32|64)\\.(a[dn]d|c[lt]z|(div|rem)_[su]|eqz?|[gl][te]_[su]|mul|ne|popcnt|rot[lr]|sh(l|r_[su])|sub|x?or)|i64\\.extend_[su]_i32|i32\\.wrap_i64|i(32|64)\\.trunc_f(32|64)_[su]|f(32|64)\\.convert_i(32|64)_[su]|f64\\.promote_f32|f32\\.demote_f64|f32\\.reinterpret_i32|i32\\.reinterpret_f32|f64\\.reinterpret_i64|i64\\.reinterpret_f64|memory(\\.((atomic\\.(notify|wait(32|64)))|grow|size))?|i64.atomic\\.(load32_u|store32|rmw32\\.(a[dn]d|sub|x?or|(cmp)?xchg)_u)|i(32|64)\\.atomic\\.(load((8|16)_u)?|store(8|16)?|rmw(\\.(a[dn]d|sub|x?or|(cmp)?xchg)|(8|16)\\.(a[dn]d|sub|x?or|(cmp)?xchg)_u))|v128\\.load(8x8|16x4|32x2)_[su]|v128\\.load(8|16|32|64)_splat|v128\\.(load|store)(8|16|32|64)_lane|v128\\.load(32|64)_zero|v128.(load|store|const|not|andnot|and|or|xor|bitselect|any_true)|i(8x16|16x8)\\.(extract_lane_[su]|(add|sub)_sat_[su]|avgr_u)|i(8x16|16x8|32x4|64x2)\\.(neg|add|sub|abs|shl|shr_[su]|all_true|bitmask|eq|ne|[lg][te]_s)|(i(8x16|16x8|32x4|64x2)|f(32x4|64x2)).(splat|replace_lane)|i(8x16|16x8|32x4)\\.(([lg][te]_u)|((min|max)_[su]))|f(32x4|64x2)\\.(neg|add|sub|abs|nearest|eq|ne|[lg][te]|sqrt|mul|div|min|max|ceil|floor|trunc)|[fi](32x4|64x2)\\.extract_lane|i8x16\\.(shuffle|swizzle|popcnt|narrow_i16x8_[su])|i16x8\\.(narrow_i32x4_[su]|mul|extadd_pairwise_i8x16_[su]|q15mulr_sat_s)|i16x8\\.(extend|extmul)_(low|high)_i8x16_[su]|i32x4\\.(mul|dot_i16x8_s|trunc_sat_f64x2_[su]_zero)|i32x4\\.((extend|extmul)_(low|high)_i16x8_|trunc_sat_f32x4_|extadd_pairwise_i16x8_)[su]|i64x2\\.(mul|(extend|extmul)_(low|high)_i32x4_[su])|f32x4\\.(convert_i32x4_[su]|demote_f64x2_zero)|f64x2\\.(promote_low_f32x4|convert_low_i32x4_[su])|\\bany\\b|array\\.len|(array|struct)(\\.(new_(default_)?with_rtt|get(_[su])?|set))?|\\beq\\b|field|i31\\.(new|get_[su])|\\bnull\\b|ref(\\.(([ai]s_(data|func|i31))|cast|eq|func|(is_|as_non_)?null|test))?|rtt(\\.(canon|sub))?",
                ignore_case: false,
                token: Some(Kind::Keyword),
                next: None,
            },
            Rule { pattern: "\\b((any|data|eq|extern|i31|func)ref|[fi](32|64)|i(8|16))\\b", ignore_case: false, token: None, next: None },
            Rule { pattern: "\\$([a-zA-Z0-9_`\\+\\-\\*\\/\\\\\\^~=<>!\\?@#$%&|:\\.]+)", ignore_case: false, token: Some(Kind::Variable), next: None },
            Rule {
                pattern: "\"(?:[^\"\\\\\\x00-\\x1f\\x7f]|\\\\[nt\\\\'\"]|\\\\[0-9a-fA-F][0-9a-fA-F])*\"",
                ignore_case: false,
                token: Some(Kind::String),
                next: None,
            },
            Rule { pattern: "\\(;.*?", ignore_case: false, token: Some(Kind::Comment), next: Some("comment") },
            Rule { pattern: ";;.*$", ignore_case: false, token: Some(Kind::Comment), next: None },
            Rule { pattern: "\\(", ignore_case: false, token: None, next: None },
            Rule { pattern: "\\)", ignore_case: false, token: None, next: None },
        ],
    ),
    (
        "comment",
        &[
            Rule { pattern: ".*?;\\)", ignore_case: false, token: Some(Kind::Comment), next: Some("start") },
            Rule { pattern: ".*", ignore_case: false, token: Some(Kind::Comment), next: None },
        ],
    ),
];
