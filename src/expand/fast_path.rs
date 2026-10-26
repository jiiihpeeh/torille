//! Lexicalised and irregular surfaces the fast path must accept.
//!
//! These are not productive suffix rules. They are closed, human-approved
//! spellings: lexicalised two-word phrases written together (`ennenkuin`,
//! `senjälkeen`), plural-only or defective paradigms (`häitä`, `harteilleen`),
//! and compounds whose head is a form the cheap stem maps do not reach
//! (`kävelykierrokselleen`, `morsiustyynyt`). The map names the head/base the
//! surface belongs to; the wasm typing path can then accept it without the
//! slow reverse walk.
//!
//! Keep the table sorted by surface: lookup is a binary search.

/// `(surface, base)` pairs. Sorted by `surface`.
const ACCEPTED: &[(&str, &str)] = &[
    ("aivankuin", "aivan"),
    ("aivotusten", "aivotus"),
    ("alastoman", "alaston"),
    ("alastomana", "alaston"),
    // The precomputed form/stem snapshots still carry the old class-34
    // geminating forms (`alasttomia`). The fixed base paradigm emits the
    // single-`t` plural, so keep the surfaces the snapshot lacks.
    ("alastomia", "alaston"),
    ("alhaaltapäin", "alhaalta"),
    ("antimia", "antimet"),
    ("avainkuin", "aivan"),
    ("edeltäkäsin", "edeltä"),
    ("edistyneemmät", "edistyä"),
    ("elttaantuneen", "elttaantua"),
    ("enemmittä", "enempi"),
    ("ennenkuin", "ennen"),
    ("erkanivat", "erkaantua"),
    ("eteemme", "eteen"),
    ("eteensä", "eteen"),
    ("haltuunsa", "haltuun"),
    ("harteilleen", "harteet"),
    ("hennonnut", "hennota"),
    ("hennonut", "hennota"),
    ("hennota", "hennota"),
    ("hiljempaa", "hiljaa"),
    ("hokasien", "honkanen"),
    ("honkasien", "honkanen"),
    ("hopeahäihin", "häät"),
    ("huostaani", "huosta"),
    ("huostaansa", "huosta"),
    ("häihimme", "häät"),
    ("häitä", "häät"),
    ("ikeniinsä", "ien"),
    ("itsekunkin", "itse"),
    ("kaarekselle", "kaares"),
    ("kaikkine", "kaikki"),
    ("kehoittavasti", "kehoittaa"),
    ("kehottavasti", "kehottaa"),
    ("kenet", "kuka"),
    ("keskenämme", "kesken"),
    ("kinkereillä", "kinkeri"),
    ("koneentapaisesti", "tapaisesti"),
    ("käräjiä", "käräjät"),
    ("käsikynkästä", "käsikynkkä"),
    ("käskevästi", "käskevä"),
    ("kävellytti", "kävellä"),
    ("kävelykierrokselleen", "kierros"),
    ("liinaompelijatar", "ompelijatar"),
    ("loitommalla", "loitolla"),
    ("lopettamaisillaan", "lopettaa"),
    ("minkätähden", "minkä"),
    ("mittaamattomien", "mittaamaton"),
    ("molemmin", "molemmat"),
    ("morsiuslahja", "lahja"),
    ("morsiusneidot", "neito"),
    ("morsiustyynyt", "tyyny"),
    ("mustuneeseen", "mustunut"),
    ("nahalla", "nahka"),
    ("niinkauan", "niin"),
    ("niinkuin", "niin"),
    ("ninkälaisia", "minkälainen"),
    ("nuhteellista", "nuhde"),
    ("nyrpistyksessä", "nyrpistys"),
    ("ohimennessään", "mennä"),
    ("osaavampi", "osata"),
    ("paheksuvasti", "paheksuva"),
    ("palpattelisit", "palpatella"),
    ("parhaan", "paras"),
    ("parhaat", "paras"),
    ("pariisilaismuodin", "muoti"),
    ("pihteihin", "pihdit"),
    ("pingoittuivat", "pingoittaa"),
    ("poikkimitalleen", "mitta"),
    ("pormestarinna", "pormestari"),
    ("raamatun", "raamattu"),
    ("rauhoittavasti", "rauhoittava"),
    ("rihvelitaulu", "taulu"),
    ("rusoittava", "rusoittaa"),
    ("saksia", "sakset"),
    ("saksien", "sakset"),
    ("saksilla", "sakset"),
    ("saksille", "sakset"),
    ("saksissa", "sakset"),
    ("saksista", "sakset"),
    ("selmallakaan", "selma"),
    ("senjälkeen", "sen"),
    ("sentähden", "sen"),
    ("seuduikseen", "seutu"),
    ("seuduiksemme", "seutu"),
    ("seuduikseni", "seutu"),
    ("seuduiksenne", "seutu"),
    ("seuduiksensa", "seutu"),
    ("seuduiksesi", "seutu"),
    ("seuduitta", "seutu"),
    ("silloinkun", "silloin"),
    ("simasuu", "suu"),
    ("sisäkohtaisesti", "sisäkohtainen"),
    ("siunatumpia", "siunata"),
    ("sointuvinta", "sointua"),
    ("suojelevasti", "suojeleva"),
    ("suurkaanin", "suurkaani"),
    ("säilikön", "säilikkö"),
    ("tamineita", "tamineet"),
    ("tapasin", "tavata"),
    ("tienpuoleen", "tie"),
    ("tikasten", "tikkaat"),
    ("tikkaiden", "tikkaat"),
    ("tikkailla", "tikkaat"),
    ("tikkaille", "tikkaat"),
    ("tikkailta", "tikkaat"),
    ("tikkaisiin", "tikkaat"),
    ("tikkaissa", "tikkaat"),
    ("tikkaista", "tikkaat"),
    ("tikkaita", "tikkaat"),
    ("tikkaitten", "tikkaat"),
    ("tiuskasi", "tiuskata"),
    ("unhotusta", "unhotus"),
    ("uupuneemmaksi", "uupua"),
    ("vaikken", "vaikka"),
    ("valjaisiin", "valjaat"),
    ("vankkurien", "vankkurit"),
    ("varuillasi", "varuilla"),
    ("ventovieraita", "vieras"),
    ("viittovan", "viittoa"),
    ("vällyihin", "vällyt"),
    ("yhtäkaikki", "kaikki"),
    ("ylhäältäpäin", "ylhäältä"),
    ("ylhäätäpäin", "ylhäältä"),
    ("yllinkyllin", "kyllin"),
    ("äkkipikaa", "pikaa"),
];

/// The accepted surface's base/head, if it is one of the lexical exceptions.
pub fn base(word: &str) -> Option<&'static str> {
    // `molemmat` is a pronominal quantifier with a defective plural-only
    // paradigm. The generic 16*H engine treats it as a singular adjective
    // (`molemmaa`), so the closed plural forms are lexical.
    if matches!(
        word,
        "molemmat"
            | "molempain"
            | "molempia"
            | "molempien"
            | "molempiin"
            | "molemmilla"
            | "molemmille"
            | "molemmilta"
            | "molemmiksi"
            | "molemmissa"
            | "molemmista"
            | "molempina"
            | "molemmitta"
            | "molempine"
            | "molemmin"
    ) {
        return Some("molemmat");
    }
    // `pihdit` is a plural-only class-5 lemma; the generic 5*F engine builds
    // singular nonsense from it. Its partitive plural is lexical.
    if word == "pihtejä" {
        return Some("pihdit");
    }
    // `selkämä` was added to the sanalista as a Kotus type-10 dialect
    // noun (Wiktionary: selkä + -mä), but the Omorfi-derived form/stem
    // snapshots predate it. Keep its case grid in the lexical supplement
    // so the extension's fast pass sees the inflected forms too.
    if matches!(
        word,
        "selkämä"
            | "selkämän"
            | "selkämää"
            | "selkämään"
            | "selkämässä"
            | "selkämästä"
            | "selkämällä"
            | "selkämältä"
            | "selkämälle"
            | "selkämänä"
            | "selkämäksi"
            | "selkämättä"
            | "selkämät"
            | "selkämien"
            | "selkämiä"
            | "selkämiin"
            | "selkämissä"
            | "selkämistä"
            | "selkämillä"
            | "selkämiltä"
            | "selkämille"
            | "selkäminä"
            | "selkämiksi"
            | "selkämittä"
            | "selkämin"
            | "selkämine"
    ) {
        return Some("selkämä");
    }

    // Comparative of the participle `perehtynyt`; the class-16 degree
    // engine does not reconstruct a verb participle's positive.
    if word == "perehtyneempi" {
        return Some("perehtynyt");
    }
    ACCEPTED
        .binary_search_by_key(&word, |(surface, _)| *surface)
        .ok()
        .map(|i| ACCEPTED[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reported_surfaces_resolve() {
        for (word, expected) in [
            ("pariisilaismuodin", "muoti"),
            ("rihvelitaulu", "taulu"),
            ("ennenkuin", "ennen"),
            ("hopeahäihin", "häät"),
            ("häitä", "häät"),
            ("liinaompelijatar", "ompelijatar"),
            ("loitommalla", "loitolla"),
            ("molemmin", "molemmat"),
            ("vällyihin", "vällyt"),
            ("morsiuslahja", "lahja"),
            ("morsiustyynyt", "tyyny"),
            ("käskevästi", "käskevä"),
            ("niinkauan", "niin"),
            ("nyrpistyksessä", "nyrpistys"),
            ("senjälkeen", "sen"),
            ("nahalla", "nahka"),
            ("kinkereillä", "kinkeri"),
            ("morsiusneidot", "neito"),
            ("rusoittava", "rusoittaa"),
            ("kävelykierrokselleen", "kierros"),
            ("unhotusta", "unhotus"),
            ("pormestarinna", "pormestari"),
            ("suojelevasti", "suojeleva"),
            ("edeltäkäsin", "edeltä"),
            ("harteilleen", "harteet"),
            ("paheksuvasti", "paheksuva"),
            ("niinkuin", "niin"),
            ("kenet", "kuka"),
            ("huostaani", "huosta"),
            ("varuillasi", "varuilla"),
            ("häihimme", "häät"),
            ("käsikynkästä", "käsikynkkä"),
            ("hiljempaa", "hiljaa"),
            ("hokasien", "honkanen"),
            ("koneentapaisesti", "tapaisesti"),
            ("äkkipikaa", "pikaa"),
            ("vaikken", "vaikka"),
            ("poikkimitalleen", "mitta"),
            ("honkasien", "honkanen"),
            ("kaikkine", "kaikki"),
            ("rauhoittavasti", "rauhoittava"),
            ("kehottavasti", "kehottaa"),
            ("kehoittavasti", "kehoittaa"),
            ("mustuneeseen", "mustunut"),
            ("honkasien", "honkanen"),
            ("hennonnut", "hennota"),
            ("hennonut", "hennota"),
            ("hennota", "hennota"),
            ("minkätähden", "minkä"),
            ("tienpuoleen", "tie"),
            ("yhtäkaikki", "kaikki"),
            ("nuhteellista", "nuhde"),
            ("ventovieraita", "vieras"),
            ("selmallakaan", "selma"),
            ("kävellytti", "kävellä"),
            ("ninkälaisia", "minkälainen"),
            ("pingoittuivat", "pingoittaa"),
            ("sentähden", "sen"),
            ("sisäkohtaisesti", "sisäkohtainen"),
            ("aivankuin", "aivan"),
            ("avainkuin", "aivan"),
            ("alhaaltapäin", "alhaalta"),
            ("ylhäätäpäin", "ylhäältä"),
            ("ylhäältäpäin", "ylhäältä"),
            ("alastomana", "alaston"),
            ("alastoman", "alaston"),
            ("alastomia", "alaston"),
            ("antimia", "antimet"),
            ("käräjiä", "käräjät"),
            ("parhaat", "paras"),
            ("valjaisiin", "valjaat"),
        ] {
            assert_eq!(base(word), Some(expected), "{word}");
        }
    }

    #[test]
    fn the_table_is_sorted() {
        assert!(ACCEPTED.windows(2).all(|w| w[0].0 < w[1].0));
    }
}
