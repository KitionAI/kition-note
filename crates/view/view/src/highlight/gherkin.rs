//! Gherkin, as CodeMirror 5's gherkin mode colors it, ported token for token: `Feature:`,
//! `Scenario:`, `Given `, … keywords where each may come, `@tags` tags, `#` comments, quoted
//! strings, `<placeholders>` variables in outlines and steps, `"""` doc strings and `|` tables
//! after a step. Only the translations of its keywords written in ASCII are listed.

use super::Kind;
use super::scan::{self, Out, Stream};

#[derive(Clone, Debug)]
pub struct Gherkin {
    table_header_line: bool,
    allow_feature: bool,
    allow_background: bool,
    allow_scenario: bool,
    allow_steps: bool,
    allow_placeholders: bool,
    allow_multiline_argument: bool,
    in_multiline_string: bool,
    in_multiline_table: bool,
    in_keyword_line: bool,
}

impl Default for Gherkin {
    fn default() -> Self {
        Gherkin {
            table_header_line: false,
            allow_feature: true,
            allow_background: false,
            allow_scenario: false,
            allow_steps: false,
            allow_placeholders: false,
            allow_multiline_argument: false,
            in_multiline_string: false,
            in_multiline_table: false,
            in_keyword_line: false,
        }
    }
}

/// Takes the first of `words` (`|` between them) the position starts with (and a `:` after
/// it, when `colon`).
fn take_keyword(stream: &mut Stream, words: &str, colon: bool) -> bool {
    let found = words.split('|').find(|word| stream.at(word) && (!colon || stream.byte(stream.pos + word.len()) == b':'));
    if let Some(word) = found {
        stream.pos += word.len() + usize::from(colon);
    }
    found.is_some()
}

pub fn line(text: &str, state: &mut Gherkin, out: &mut Out) {
    scan::run(text, out, |stream| token(stream, state));
}

fn token(stream: &mut Stream, state: &mut Gherkin) -> Option<Kind> {
    if stream.sol() {
        state.in_keyword_line = false;
        if state.in_multiline_table {
            state.table_header_line = false;
            let first = stream.rest().iter().find(|byte| !byte.is_ascii_whitespace());
            if first != Some(&b'|') {
                state.allow_multiline_argument = false;
                state.in_multiline_table = false;
            }
        }
    }
    stream.eat_space();
    if state.allow_multiline_argument {
        if state.in_multiline_string {
            if stream.take("\"\"\"") {
                state.in_multiline_string = false;
                state.allow_multiline_argument = false;
            } else {
                stream.skip_to_end();
            }
            return Some(Kind::String);
        }
        if state.in_multiline_table {
            if stream.take("|") {
                stream.eat_while(|byte| byte.is_ascii_whitespace());
                return Some(Kind::Punctuation);
            }
            stream.eat_while(|byte| byte != b'|');
            return if state.table_header_line { None } else { Some(Kind::String) };
        }
        if stream.take("\"\"\"") {
            state.in_multiline_string = true;
            return Some(Kind::String);
        }
        if stream.take("|") {
            state.in_multiline_table = true;
            state.table_header_line = true;
            return Some(Kind::Punctuation);
        }
    }
    if stream.take("#") {
        stream.skip_to_end();
        return Some(Kind::Comment);
    }
    if !state.in_keyword_line && stream.peek() == b'@' && stream.byte(stream.pos + 1) > b' ' {
        stream.eat_while(|byte| byte > b' ');
        return Some(Kind::Operator);
    }
    if !state.in_keyword_line && state.allow_feature && take_keyword(stream, FEATURE, true) {
        state.allow_scenario = true;
        state.allow_background = true;
        state.allow_placeholders = false;
        state.allow_steps = false;
        state.allow_multiline_argument = false;
        state.in_keyword_line = true;
        return Some(Kind::Keyword);
    }
    if !state.in_keyword_line && state.allow_background && take_keyword(stream, BACKGROUND, true) {
        state.allow_placeholders = false;
        state.allow_steps = true;
        state.allow_background = false;
        state.allow_multiline_argument = false;
        state.in_keyword_line = true;
        return Some(Kind::Keyword);
    }
    if !state.in_keyword_line && state.allow_scenario && take_keyword(stream, OUTLINE, true) {
        state.allow_placeholders = true;
        state.allow_steps = true;
        state.allow_multiline_argument = false;
        state.in_keyword_line = true;
        return Some(Kind::Keyword);
    }
    if state.allow_scenario && take_keyword(stream, EXAMPLES, true) {
        state.allow_placeholders = false;
        state.allow_steps = true;
        state.allow_background = false;
        state.allow_multiline_argument = true;
        return Some(Kind::Keyword);
    }
    if !state.in_keyword_line && state.allow_scenario && take_keyword(stream, SCENARIO, true) {
        state.allow_placeholders = false;
        state.allow_steps = true;
        state.allow_background = false;
        state.allow_multiline_argument = false;
        state.in_keyword_line = true;
        return Some(Kind::Keyword);
    }
    if !state.in_keyword_line && state.allow_steps && take_keyword(stream, STEPS, false) {
        state.allow_placeholders = true;
        state.allow_multiline_argument = true;
        state.in_keyword_line = true;
        return Some(Kind::Keyword);
    }
    if stream.take("\"") {
        stream.eat_while(|byte| byte != b'"');
        stream.eat(|byte| byte == b'"');
        return Some(Kind::String);
    }
    if state.allow_placeholders && stream.take("<") {
        stream.eat_while(|byte| byte != b'>');
        stream.eat(|byte| byte == b'>');
        return Some(Kind::Variable);
    }
    stream.next();
    stream.eat_while(|byte| !b"@\"<#".contains(&byte));
    None
}

/// `Feature:` and its translations the mode knows, those written in ASCII, in its order.
const FEATURE: &str = "Trajto|Pretty much|Potrzeba biznesowa|Osobina|Ominaisuus|Omadus|OH HAI|Mogucnost|Hwaet|Funkcja|Funkcionalnost|Funkcia|\
Fungsi|Functionaliteit|Functionalitate|Funcionalitat|Funcionalidade|Fitur|Feature|Eiginleiki|Egenskap|Egenskab|\
Caracteristica|Business Need|Aspekt|Arwedd|Ahoy matey!|Ability";

/// `Background:` and its translations the mode knows, those written in ASCII, in its order.
const BACKGROUND: &str = "Yo-ho-ho|Tausta|Taust|Rerefons|Pozadina|Pozadie|Osnova|Latar Belakang|Kontext|Konteksts|Kontekstas|Kontekst|\
Hannergrond|Grundlage|Fundo|Fono|First off|Dis is what went down|Dasar|Contexto|Contexte|Context|Contesto|\
Cenario de Fundo|Cefndir|Bakgrunnur|Bakgrunn|Bakgrund|Baggrund|Background|B4|Antecedents|Antecedentes|Aer|Achtergrond";

/// `Scenario Outline:` and its translations the mode knows, those written in ASCII, in its order.
const OUTLINE: &str = "Wharrimean is|Template Situai|Template Senario|Template Keadaan|Tapausaihio|Szenariogrundriss|Szablon scenariusza|\
Swa hwaer swa|Struktura scenarija|Structura scenariu|Skica|Skenario konsep|Shiver me timbers|Schema dello scenario|\
Scenariomall|Scenariomal|Scenario Template|Scenario Outline|Scenario Amlinellol|Reckon it's like|Raamstsenaarium|\
Plang vum Szenario|MISHUN SRSLY|Menggariskan Senario|Konturo de la scenaro|Koncept|Esquema do Cenario|\
Esquema del escenario|Esquema de l'escenari|Esbozo do escenario|Delineacao do Cenario|All y'all|Abstrakt Scenario|\
Abstract Scenario";

/// `Examples:` and its translations the mode knows, those written in ASCII, in its order.
const EXAMPLES: &str = "You'll wanna|Voorbeelden|Variantai|Tapaukset|Se the|Scenarios|Scenariji|Scenarijai|Primjeri|Primeri|Paraugs|Juhtumid|\
Exemplos|Exemples|Exemple|Exempel|EXAMPLZ|Examples|Esempi|Enghreifftiau|Ekzemploj|Eksempler|Ejemplos|\
Dead men tell no tales|Contoh|Cenarios|Beispiller|Beispiele";

/// `Scenario:` and its translations the mode knows, those written in ASCII, in its order.
const SCENARIO: &str = "The thing of it is|Tapaus|Szenario|Swa|Stsenaarium|Skenario|Situai|Senaryo|Senario|Scenaro|Scenariusz|Scenariu|\
Scenario|Scenarijus|Scenarij|Scenarie|Primer|MISHUN|Keadaan|Heave to|Escenario|Escenari|Cenario|Awww, look mate";

/// `Given `, `When `, `Then `, … and its translations the mode knows, those written in ASCII, in its order.
const STEPS: &str = "Zatati |Zadato |Zadate |Zadano |Zadani |Zadan |Za predpokladu |Youse know when youse got |Youse know like when |Yna |\
Yeah nah |Y'know |Y |Wun |Wtedy |When y'all |When |Wenn |WEN |wann |Ve |Und |Un |ugeholl |Too right |Thurh |\
Then y'all |Then |Tha the |Tha |Tetapi |Tapi |Tak |Tada |Tad |Stel |Soit |Siis |Si |Sed |Se |Quando |Quand |Quan |\
Pryd |Potom |Pokud |Pero |Pak |Oraz |Onda |Ond |Oletetaan |Og |Och |O zaman |Niin |Mutta |Men |Mas |Maka |Majd |Mais |\
Maar |Ma |Lorsque |Lorsqu'|Logo |Let go and haul |Kun |Kuid |Kui |Kiedy |Khi |Ketika |Kemudian |Kaj |Kai |Kada |Kad |\
Ja |It's just unbelievable |Ir |I CAN HAZ |I |Ha |Givun |Givet |Given y'all |Given |Gitt |Gegeven |Gegeben seien |\
Gegeben sei |Gdy |Gangway! |Fakat |Et |Entonces |Entao |En |Ef |Eeldades |E |Duota |Dun |Donat |Donada |Do |\
Diyelim ki |Diberi |Dengan |Den youse gotta |DEN |De |Dato |Dati fiind |Dati |Date fiind |Date |Data |Dat fiind |Dar |\
Dann |dann |Dan |Dados |Dado |Dadas |Dada |Cuando |Cho |Cando |Cand |Cal |But y'all |\
But at the end of the day I reckon |BUT |But |Buh |Blimey! |Bet |Bagi |Aye |awer |Avast! |Atunci |Atesa |Apabila |\
Anrhegedig a |Angenommen |And y'all |And |AN |An |an |Amikor |Amennyiben |Ama |Als |Alors |Allora |Ali |Aleshores |\
Ale |Akkor |Ak |Adott |Ac |Aber |A |a |7 |* ";
