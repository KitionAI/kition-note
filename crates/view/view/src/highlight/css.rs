//! CSS, as CodeMirror 5's css mode colors it: in a selector a tag is a tag, `.class` a qualifier,
//! `#id` a builtin, `:hover` a variable; in a block a known property is a property; in a value a
//! keyword is an atom, a color's name a keyword, a function a variable, a number (with its unit)
//! a number, a hex color an atom; an at-rule a definition.

use super::Kind;
use super::scan::Out;

/// Where a token is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Context {
    /// Selectors (at the top, or inside an at-rule's block).
    Top,
    /// A rule's declarations.
    Block,
    /// A declaration's value.
    Value,
    /// An at-rule's prelude (a media query).
    At,
    /// An attribute selector's `[…]`.
    Attribute,
    /// A value's `(…)`.
    Parens,
}

#[derive(Clone, Debug)]
pub struct Css {
    stack: Vec<Context>,
    comment: bool,
    /// The at-rule's prelude ends in a block of declarations (`@font-face`), not of rules.
    at_declarations: bool,
    /// The at-rule is `@keyframes`.
    keyframes: bool,
}

impl Default for Css {
    fn default() -> Self {
        Css { stack: vec![Context::Top], comment: false, at_declarations: false, keyframes: false }
    }
}

const PROPERTIES: &str = "align-content align-items align-self alignment-adjust alignment-baseline all anchor-point animation animation-delay \
animation-direction animation-duration animation-fill-mode animation-iteration-count animation-name animation-play-state animation-timing-function \
appearance aspect-ratio azimuth backdrop-filter backface-visibility background background-attachment background-blend-mode background-clip \
background-color background-image background-origin background-position background-position-x background-position-y background-repeat \
background-size baseline-shift binding bleed block-size bookmark-label bookmark-level bookmark-state bookmark-target border border-block \
border-block-color border-block-end border-block-start border-block-style border-block-width border-bottom border-bottom-color \
border-bottom-left-radius border-bottom-right-radius border-bottom-style border-bottom-width border-collapse border-color border-image \
border-image-outset border-image-repeat border-image-slice border-image-source border-image-width border-inline border-inline-color \
border-inline-end border-inline-start border-inline-style border-inline-width border-left border-left-color border-left-style \
border-left-width border-radius border-right border-right-color border-right-style border-right-width border-spacing border-style \
border-top border-top-color border-top-left-radius border-top-right-radius border-top-style border-top-width border-width bottom \
box-decoration-break box-shadow box-sizing break-after break-before break-inside caption-side caret-color clear clip clip-path color \
color-profile color-scheme column-count column-fill column-gap column-rule column-rule-color column-rule-style column-rule-width column-span \
column-width columns contain content counter-increment counter-reset crop cue cue-after cue-before cursor direction display \
dominant-baseline drop-initial-after-adjust drop-initial-after-align drop-initial-before-adjust drop-initial-before-align drop-initial-size \
drop-initial-value elevation empty-cells fit fit-content fit-position flex flex-basis flex-direction flex-flow flex-grow flex-shrink \
flex-wrap float float-offset flow-from flow-into font font-family font-feature-settings font-kerning font-language-override \
font-optical-sizing font-size font-size-adjust font-stretch font-style font-synthesis font-variant font-variant-alternates \
font-variant-caps font-variant-east-asian font-variant-ligatures font-variant-numeric font-variant-position font-variation-settings \
font-weight gap grid grid-area grid-auto-columns grid-auto-flow grid-auto-rows grid-column grid-column-end grid-column-gap \
grid-column-start grid-gap grid-row grid-row-end grid-row-gap grid-row-start grid-template grid-template-areas grid-template-columns \
grid-template-rows hanging-punctuation height hyphens icon image-orientation image-rendering image-resolution inline-box-align inset \
inset-block inset-block-end inset-block-start inset-inline inset-inline-end inset-inline-start isolation justify-content justify-items \
justify-self left letter-spacing line-break line-height line-height-step line-stacking line-stacking-ruby line-stacking-shift \
line-stacking-strategy list-style list-style-image list-style-position list-style-type margin margin-block margin-block-end \
margin-block-start margin-bottom margin-inline margin-inline-end margin-inline-start margin-left margin-right margin-top marks \
marquee-direction marquee-loop marquee-play-count marquee-speed marquee-style mask-clip mask-composite mask-image mask-mode mask-origin \
mask-position mask-repeat mask-size mask-type max-block-size max-height max-inline-size max-width min-block-size min-height \
min-inline-size min-width mix-blend-mode move-to nav-down nav-index nav-left nav-right nav-up object-fit object-position offset \
offset-anchor offset-distance offset-path offset-position offset-rotate opacity order orphans outline outline-color outline-offset \
outline-style outline-width overflow overflow-style overflow-wrap overflow-x overflow-y padding padding-block padding-block-end \
padding-block-start padding-bottom padding-inline padding-inline-end padding-inline-start padding-left padding-right padding-top page \
page-break-after page-break-before page-break-inside page-policy pause pause-after pause-before perspective perspective-origin pitch \
pitch-range place-content place-items place-self play-during position presentation-level punctuation-trim quotes region-break-after \
region-break-before region-break-inside region-fragment rendering-intent resize rest rest-after rest-before richness right rotate \
rotation rotation-point row-gap ruby-align ruby-overhang ruby-position ruby-span scale scroll-behavior scroll-margin scroll-margin-block \
scroll-margin-block-end scroll-margin-block-start scroll-margin-bottom scroll-margin-inline scroll-margin-inline-end \
scroll-margin-inline-start scroll-margin-left scroll-margin-right scroll-margin-top scroll-padding scroll-padding-block \
scroll-padding-block-end scroll-padding-block-start scroll-padding-bottom scroll-padding-inline scroll-padding-inline-end \
scroll-padding-inline-start scroll-padding-left scroll-padding-right scroll-padding-top scroll-snap-align scroll-snap-type \
shape-image-threshold shape-inside shape-margin shape-outside size speak speak-as speak-header speak-numeral speak-punctuation \
speech-rate stress string-set tab-size table-layout target target-name target-new target-position text-align text-align-last \
text-combine-upright text-decoration text-decoration-color text-decoration-line text-decoration-skip text-decoration-skip-ink \
text-decoration-style text-emphasis text-emphasis-color text-emphasis-position text-emphasis-style text-height text-indent \
text-justify text-orientation text-outline text-overflow text-rendering text-shadow text-size-adjust text-space-collapse \
text-transform text-underline-position text-wrap top touch-action transform transform-origin transform-style transition \
transition-delay transition-duration transition-property transition-timing-function translate unicode-bidi user-select \
vertical-align visibility voice-balance voice-duration voice-family voice-pitch voice-range voice-rate voice-stress voice-volume \
volume white-space widows width will-change word-break word-spacing word-wrap writing-mode z-index clip-rule color-interpolation \
color-interpolation-filters color-rendering fill fill-opacity fill-rule filter flood-color flood-opacity glyph-orientation-horizontal \
glyph-orientation-vertical lighting-color marker marker-end marker-mid marker-start paint-order shape-rendering stop-color \
stop-opacity stroke stroke-dasharray stroke-dashoffset stroke-linecap stroke-linejoin stroke-miterlimit stroke-opacity stroke-width \
text-anchor writing-mode accent-color src unicode-range font-display";

const NON_STANDARD: &str = "accent-color aspect-ratio border-block-end-color border-block-end-style border-block-end-width \
border-block-start-color border-block-start-style border-block-start-width border-inline-end-color border-inline-end-style \
border-inline-end-width border-inline-start-color border-inline-start-style border-inline-start-width content-visibility \
margin-trim overscroll-behavior overscroll-behavior-x overscroll-behavior-y scrollbar-arrow-color scrollbar-base-color \
scrollbar-dark-shadow-color scrollbar-face-color scrollbar-gutter scrollbar-highlight-color scrollbar-shadow-color \
scrollbar-track-color scrollbar-width searchfield-cancel-button searchfield-decoration searchfield-results-button \
searchfield-results-decoration shape-inside zoom";

const VALUES: &str = "above absolute activeborder additive activecaption afar after-white-space ahead alias all all-scroll alphabetic \
alternate always amharic amharic-abegede antialiased appworkspace arabic-indic armenian asterisks attr auto auto-flow avoid \
avoid-column avoid-page avoid-region axis-pan background backwards baseline below bidi-override binary bengali blink block \
block-axis blur bold bolder border border-box both bottom break break-all break-word brightness bullets button buttonface \
buttonhighlight buttonshadow buttontext calc cambodian capitalize caps-lock-indicator caption captiontext caret cell center checkbox \
circle cjk-decimal cjk-earthly-branch cjk-heavenly-stem cjk-ideographic clear clip close-quote col-resize collapse color color-burn \
color-dodge column column-reverse compact condensed contain content contents content-box context-menu continuous contrast copy \
counter counters cover crop cross crosshair cubic-bezier currentcolor cursive cyclic darken dashed decimal decimal-leading-zero default \
default-button dense destination-atop destination-in destination-out destination-over devanagari difference disc discard disclosure-closed \
disclosure-open document dot-dash dot-dot-dash dotted double down drop-shadow e-resize ease ease-in ease-in-out ease-out element \
ellipse ellipsis embed end ethiopic ethiopic-abegede exclusion expanded extends extra-condensed extra-expanded fantasy fast fill \
fill-box fixed flat flex flex-end flex-start footnotes forwards from geometricPrecision georgian grayscale graytext grid groove \
gujarati gurmukhi hand hangul hangul-consonant hard-light hebrew help hidden hide higher highlight highlighttext hiragana \
hiragana-iroha horizontal hsl hsla hue hue-rotate icon ignore inactiveborder inactivecaption inactivecaptiontext infinite \
infobackground infotext inherit initial inline inline-axis inline-block inline-flex inline-grid inline-table inset inside intrinsic \
invert italic japanese-formal japanese-informal justify kannada katakana katakana-iroha keep-all khmer korean-hangul-formal \
korean-hanja-formal korean-hanja-informal landscape lao large larger left level lighter lighten line-through linear linear-gradient \
lines list-item listbox listitem local logical loud lower lower-alpha lower-armenian lower-greek lower-hexadecimal lower-latin \
lower-norwegian lower-roman lowercase ltr luminosity malayalam manipulation match matrix matrix3d media-play-button \
media-slider media-sliderthumb media-volume-slider medium menu menulist menulist-button menutext message-box middle min-intrinsic \
mix mongolian monospace move multiple multiple_mask_images multiply myanmar n-resize narrower ne-resize nesw-resize no-close-quote \
no-drop no-open-quote no-repeat none normal not-allowed nowrap ns-resize numbers numeric nw-resize nwse-resize oblique octal \
opacity open-quote optimizeLegibility optimizeSpeed oriya oromo outset outside outside-shape overlay overline padding padding-box \
painted page paused persian perspective pinch-zoom plus-darker plus-lighter pointer polygon portrait pre pre-line pre-wrap \
preserve-3d progress push-button radial-gradient radio read-only read-write read-write-plaintext-only rectangle region relative \
repeat repeating-linear-gradient repeating-radial-gradient repeat-x repeat-y reset reverse rgb rgba ridge right rotate rotate3d \
rotateX rotateY rotateZ round row row-resize row-reverse rtl run-in running s-resize sans-serif saturate saturation scale scale3d \
scaleX scaleY scaleZ screen scroll scrollbar scroll-position se-resize searchfield searchfield-cancel-button searchfield-decoration \
searchfield-results-button searchfield-results-decoration self-start self-end semi-condensed semi-expanded separate sepia serif \
show sidama simp-chinese-formal simp-chinese-informal single skew skewX skewY skip-white-space slide slider-horizontal \
slider-vertical sliderthumb-horizontal sliderthumb-vertical slow small small-caps small-caption smaller soft-light solid somali \
source-atop source-in source-out source-over space space-around space-between space-evenly spell-out square square-button start \
static status-bar stretch stroke stroke-box sub subpixel-antialiased svg_masks super sw-resize symbolic symbols system-ui table \
table-caption table-cell table-column table-column-group table-footer-group table-header-group table-row table-row-group tamil \
telugu text text-bottom text-top textarea textfield thai thick thin threeddarkshadow threedface threedhighlight threedlightshadow \
threedshadow tibetan tigre tigrinya-er tigrinya-er-abegede tigrinya-et tigrinya-et-abegede to top trad-chinese-formal \
trad-chinese-informal transform translate translate3d translateX translateY translateZ transparent ultra-condensed ultra-expanded \
underline unidirectional-pan unset up upper-alpha upper-armenian upper-greek upper-hexadecimal upper-latin upper-norwegian \
upper-roman uppercase urdu url var vertical vertical-text view-box visible visibleFill visiblePainted visibleStroke visual w-resize \
wait wave wider window windowframe windowtext words wrap wrap-reverse x-large x-small xor xx-large xx-small";

const COLORS: &str = "aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood \
cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen \
darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray \
darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia \
gainsboro ghostwhite gold goldenrod gray grey green greenyellow honeydew hotpink indianred indigo ivory khaki lavender lavenderblush \
lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon \
lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon \
mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise \
mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid \
palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red \
rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray slategrey snow \
springgreen steelblue tan teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen";

const MEDIA_TYPES: &str = "all aural braille handheld print projection screen tty tv embossed";
const MEDIA_FEATURES: &str = "width min-width max-width height min-height max-height device-width min-device-width max-device-width \
device-height min-device-height max-device-height aspect-ratio min-aspect-ratio max-aspect-ratio device-aspect-ratio \
min-device-aspect-ratio max-device-aspect-ratio color min-color max-color color-index min-color-index max-color-index monochrome \
min-monochrome max-monochrome resolution min-resolution max-resolution scan grid orientation device-pixel-ratio \
min-device-pixel-ratio max-device-pixel-ratio pointer any-pointer hover any-hover prefers-color-scheme dynamic-range \
video-dynamic-range prefers-reduced-motion prefers-contrast";
const MEDIA_VALUES: &str = "landscape portrait none coarse fine on-demand hover interlace progressive dark light standard high";

fn listed(list: &str, word: &str) -> bool {
    list.split_ascii_whitespace().any(|known| known == word)
}

fn byte(bytes: &[u8], at: usize) -> u8 {
    bytes.get(at).copied().unwrap_or(0)
}

fn word_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-' || byte == b'\\' || byte >= 0x80
}

impl Css {
    fn context(&self) -> Context {
        self.stack.last().copied().unwrap_or(Context::Top)
    }
}

/// A line of CSS (`line_comments`: SCSS's and Less's `//` comments too).
pub fn line(text: &str, css: &mut Css, line_comments: bool, out: &mut Out) {
    let bytes = text.as_bytes();
    let mut at = 0;
    if css.comment {
        match text.find("*/") {
            Some(found) => {
                out.push(0..found + 2, Kind::Comment);
                css.comment = false;
                at = found + 2;
            }
            None => {
                out.push(0..text.len(), Kind::Comment);
                return;
            }
        }
    }
    while at < bytes.len() {
        let here = byte(bytes, at);
        let start = at;
        let context = css.context();
        match here {
            b' ' | b'\t' | b'\r' => at += 1,
            b'/' if byte(bytes, at + 1) == b'*' => match text.get(at + 2..).and_then(|rest| rest.find("*/")) {
                Some(found) => {
                    out.push(at..at + found + 4, Kind::Comment);
                    at += found + 4;
                }
                None => {
                    out.push(at..bytes.len(), Kind::Comment);
                    css.comment = true;
                    return;
                }
            },
            b'/' if line_comments && byte(bytes, at + 1) == b'/' => {
                out.push(at..bytes.len(), Kind::Comment);
                return;
            }
            b'"' | b'\'' => {
                at = super::scan::string_end(bytes, at, here).unwrap_or(bytes.len());
                out.push(start..at, Kind::String);
            }
            b'@' => {
                at = scan_word(bytes, at + 1);
                let rule = text.get(start + 1..at).unwrap_or("").to_ascii_lowercase();
                css.at_declarations = matches!(rule.as_str(), "font-face" | "page" | "counter-style" | "property" | "font-feature-values");
                css.keyframes = rule.ends_with("keyframes");
                css.stack.push(Context::At);
            }
            b'#' => {
                at = scan_word(bytes, at + 1);
                out.mark(start..at, if matches!(context, Context::Top | Context::At) { Some(Kind::Property) } else { None });
            }
            b'!' => {
                at += 1;
                while matches!(byte(bytes, at), b' ' | b'\t') {
                    at += 1;
                }
                while byte(bytes, at).is_ascii_alphanumeric() || byte(bytes, at) == b'_' {
                    at += 1;
                }
                out.push(start..at, Kind::Keyword);
            }
            b'0'..=b'9' => {
                at = number(bytes, at);
                out.push(start..at, Kind::Number);
            }
            b'.' if byte(bytes, at + 1).is_ascii_digit() => {
                at = number(bytes, at);
                out.push(start..at, Kind::Number);
            }
            b'-' if byte(bytes, at + 1).is_ascii_digit() || (byte(bytes, at + 1) == b'.' && byte(bytes, at + 2).is_ascii_digit()) => {
                at = number(bytes, at + 1);
                out.push(start..at, Kind::Number);
            }
            // A vendor prefix (`-webkit-`) is meta; the name after it goes on as a word.
            b'-' if byte(bytes, at + 1).is_ascii_alphabetic() && vendor(bytes, at).is_some() => {
                at = vendor(bytes, at).unwrap_or(at + 1);
                out.push(start..at, Kind::Comment);
            }
            b'-' if byte(bytes, at + 1) == b'-' => {
                // A custom property: a variable, defined or used.
                at = scan_word(bytes, at);
                out.push(start..at, Kind::Variable);
                if context == Context::Block && super::scan::followed_by(bytes, at, b':') {
                    css.stack.push(Context::Value);
                    at = colon_after(bytes, at);
                }
            }
            b'.' if matches!(context, Context::Top | Context::At)
                && (byte(bytes, at + 1).is_ascii_alphabetic() || matches!(byte(bytes, at + 1), b'_' | b'-')) =>
            {
                at = scan_word(bytes, at + 1);
                out.push(start..at, Kind::String);
            }
            b':' => {
                at += 1;
                match context {
                    Context::Top | Context::At => {
                        // A pseudo-class or -element.
                        if byte(bytes, at) == b':' {
                            at += 1;
                        }
                        let end = scan_word(bytes, at);
                        out.push(at..end, Kind::Variable);
                        at = end;
                    }
                    Context::Block => css.stack.push(Context::Value),
                    _ => {}
                }
            }
            b';' => {
                at += 1;
                match context {
                    Context::Value => {
                        css.stack.pop();
                    }
                    Context::At => {
                        css.stack.pop();
                    }
                    _ => {}
                }
            }
            b'{' => {
                at += 1;
                if context == Context::Value {
                    css.stack.pop();
                }
                match css.context() {
                    Context::At => {
                        css.stack.pop();
                        let declarations = std::mem::take(&mut css.at_declarations);
                        css.stack.push(if declarations { Context::Block } else { Context::Top });
                    }
                    Context::Top => css.stack.push(Context::Block),
                    _ => css.stack.push(Context::Block),
                }
            }
            b'}' => {
                at += 1;
                if css.context() == Context::Value {
                    css.stack.pop();
                }
                if css.stack.len() > 1 {
                    css.stack.pop();
                }
            }
            b'[' if context == Context::Top => {
                at += 1;
                css.stack.push(Context::Attribute);
            }
            b']' if context == Context::Attribute => {
                at += 1;
                css.stack.pop();
            }
            b'(' => {
                at += 1;
                if matches!(context, Context::Value | Context::Parens) {
                    css.stack.push(Context::Parens);
                }
            }
            b')' => {
                at += 1;
                if context == Context::Parens {
                    css.stack.pop();
                }
            }
            _ if word_char(here) => {
                at = scan_word(bytes, at);
                // A function: `rgba(`, `url(` (its address a string).
                if byte(bytes, at) == b'(' {
                    out.push(start..at, Kind::Variable);
                    let name = text.get(start..at).unwrap_or("").to_ascii_lowercase();
                    if matches!(name.as_str(), "url" | "url-prefix" | "domain" | "regexp") {
                        let close = text.get(at..).and_then(|rest| rest.find(')')).map_or(bytes.len(), |found| at + found);
                        out.push(at + 1..close, Kind::String);
                        at = (close + 1).min(bytes.len());
                    }
                    continue;
                }
                let word = text.get(start..at).unwrap_or("").to_ascii_lowercase();
                // A `@keyframes` rule's name is a variable.
                let kind = if context == Context::At && css.keyframes { Some(Kind::Variable) } else { word_kind(context, &word) };
                out.mark(start..at, kind);
                if context == Context::Block && super::scan::followed_by(bytes, at, b':') {
                    css.stack.push(Context::Value);
                    at = colon_after(bytes, at);
                }
            }
            _ => at += 1,
        }
    }
}

/// Past the `:` after a property (its spaces before it).
fn colon_after(bytes: &[u8], at: usize) -> usize {
    super::scan::next_significant(bytes, at).map_or(at, |(colon, _)| colon + 1)
}

/// What a word is where it is.
fn word_kind(context: Context, word: &str) -> Option<Kind> {
    match context {
        Context::Top => Some(Kind::Operator),
        Context::Attribute => Some(Kind::Operator),
        Context::Block => {
            if listed(PROPERTIES, word) {
                Some(Kind::Property)
            } else if listed(NON_STANDARD, word) {
                Some(Kind::String)
            } else {
                // An unknown property is the mode's error.
                Some(Kind::Operator)
            }
        }
        Context::Value | Context::Parens => {
            if listed(VALUES, word) {
                None
            } else if listed(COLORS, word) {
                Some(Kind::Keyword)
            } else {
                Some(Kind::Variable)
            }
        }
        Context::At => {
            if matches!(word, "only" | "not" | "and" | "or") {
                Some(Kind::Keyword)
            } else if listed(MEDIA_TYPES, word) || listed(MEDIA_FEATURES, word) || listed(PROPERTIES, word) {
                Some(Kind::Property)
            } else if listed(MEDIA_VALUES, word) || listed(COLORS, word) {
                Some(Kind::Keyword)
            } else if listed(NON_STANDARD, word) {
                Some(Kind::String)
            } else if listed(VALUES, word) {
                None
            } else {
                Some(Kind::Operator)
            }
        }
    }
}

/// The end of a vendor prefix at `at` (`-webkit-`), if one starts there.
fn vendor(bytes: &[u8], at: usize) -> Option<usize> {
    let mut end = at + 1;
    while byte(bytes, end).is_ascii_alphanumeric() || byte(bytes, end) == b'_' {
        end += 1;
    }
    (end > at + 1 && byte(bytes, end) == b'-').then_some(end + 1)
}

fn scan_word(bytes: &[u8], at: usize) -> usize {
    let mut end = at;
    while word_char(byte(bytes, end)) {
        end += 1;
    }
    end
}

/// The end of a number at `at`, its unit or `%` with it.
fn number(bytes: &[u8], at: usize) -> usize {
    let mut end = at;
    while byte(bytes, end).is_ascii_alphanumeric() || matches!(byte(bytes, end), b'.' | b'%' | b'_') {
        end += 1;
    }
    end
}
