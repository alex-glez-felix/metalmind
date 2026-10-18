//! El tipo de un adjunto por sus primeros bytes, sin creerle al portapapeles
//! ni a la extensión que traiga. Solo formatos con una firma inequívoca: un
//! `.zip` puede ser un `.docx` o un `.epub`, así que no se adivina.

/// Hasta dónde puede llegar el prólogo de un SVG antes de `<svg`.
const SVG_PROBE: usize = 1024;

/// Las marcas principales de `ftyp` que son MP4; otras (`3gp4`, `M4V `,
/// `crx `) son formatos propios con la misma caja.
const MP4_BRANDS: &[&[u8; 4]] = &[
    b"isom", b"iso2", b"iso3", b"iso4", b"iso5", b"iso6", b"iso8", b"iso9", b"mp41", b"mp42",
    b"avc1", b"dash",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Png,
    Jpeg,
    Gif,
    Webp,
    Bmp,
    Tiff,
    Avif,
    Heic,
    Svg,
    Pdf,
    Mp4,
    Mov,
    M4a,
    Webm,
    Mkv,
    Avi,
    Mp3,
    Ogg,
    Wav,
    Flac,
}

impl FileKind {
    /// El tipo de `bytes`, o `None` si no se reconoce.
    #[must_use]
    pub fn detect(bytes: &[u8]) -> Option<Self> {
        signature(bytes)
            .or_else(|| riff(bytes))
            .or_else(|| iso_media(bytes))
            .or_else(|| matroska(bytes))
            .or_else(|| svg(bytes))
    }

    /// Si otros formatos empiezan igual: los RAW de cámara (`.cr2`, `.nef`,
    /// `.dng`, …) son TIFF por dentro. Para esos, la extensión propia gana.
    #[must_use]
    pub fn shares_signature(self) -> bool {
        self == Self::Tiff
    }

    /// La extensión, sin punto y en minúsculas.
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
            Self::Avif => "avif",
            Self::Heic => "heic",
            Self::Svg => "svg",
            Self::Pdf => "pdf",
            Self::Mp4 => "mp4",
            Self::Mov => "mov",
            Self::M4a => "m4a",
            Self::Webm => "webm",
            Self::Mkv => "mkv",
            Self::Avi => "avi",
            Self::Mp3 => "mp3",
            Self::Ogg => "ogg",
            Self::Wav => "wav",
            Self::Flac => "flac",
        }
    }
}

/// Los formatos que se reconocen por un prefijo fijo.
fn signature(bytes: &[u8]) -> Option<FileKind> {
    const PREFIXES: &[(&[u8], FileKind)] = &[
        (b"\x89PNG\r\n\x1a\n", FileKind::Png),
        (b"\xff\xd8\xff", FileKind::Jpeg),
        (b"GIF87a", FileKind::Gif),
        (b"GIF89a", FileKind::Gif),
        (b"II*\0", FileKind::Tiff),
        (b"MM\0*", FileKind::Tiff),
        (b"%PDF-", FileKind::Pdf),
        (b"ID3", FileKind::Mp3),
        (b"\xff\xfb", FileKind::Mp3),
        (b"\xff\xf3", FileKind::Mp3),
        (b"\xff\xf2", FileKind::Mp3),
        (b"OggS", FileKind::Ogg),
        (b"fLaC", FileKind::Flac),
    ];
    let found = PREFIXES
        .iter()
        .find(|(prefix, _)| bytes.starts_with(prefix))
        .map(|&(_, kind)| kind);
    // `BM` solo son dos bytes: los cuatro reservados de la cabecera van en cero.
    found.or_else(|| {
        (bytes.starts_with(b"BM") && bytes.get(6..10) == Some(&[0; 4])).then_some(FileKind::Bmp)
    })
}

fn riff(bytes: &[u8]) -> Option<FileKind> {
    if !bytes.starts_with(b"RIFF") {
        return None;
    }
    match bytes.get(8..12)? {
        b"WEBP" => Some(FileKind::Webp),
        b"WAVE" => Some(FileKind::Wav),
        b"AVI " => Some(FileKind::Avi),
        _ => None,
    }
}

/// La caja `ftyp` de ISO BMFF: la marca principal y las compatibles dicen
/// si es imagen (AVIF, HEIC), audio o video.
fn iso_media(bytes: &[u8]) -> Option<FileKind> {
    if bytes.get(4..8)? != b"ftyp" {
        return None;
    }
    let size = usize::try_from(u32::from_be_bytes(bytes.get(0..4)?.try_into().ok()?)).ok()?;
    let major = bytes.get(8..12)?;
    let compatible = bytes.get(16..size.min(bytes.len())).unwrap_or_default();
    let (compatible, _) = compatible.as_chunks::<4>();
    let brands: Vec<&[u8]> = std::iter::once(major)
        .chain(compatible.iter().map(<[u8; 4]>::as_slice))
        .collect();
    let has = |wanted: &[&[u8]]| brands.iter().any(|brand| wanted.contains(brand));
    if has(&[b"avif", b"avis"]) {
        Some(FileKind::Avif)
    } else if has(&[
        b"heic", b"heix", b"heim", b"heis", b"hevc", b"hevx", b"mif1", b"msf1",
    ]) {
        Some(FileKind::Heic)
    } else if major == b"M4A " {
        Some(FileKind::M4a)
    } else if major == b"qt  " {
        Some(FileKind::Mov)
    } else if MP4_BRANDS.iter().any(|brand| major == brand.as_slice()) {
        Some(FileKind::Mp4)
    } else {
        None
    }
}

/// Un `.webm` es un Matroska cuyo tipo en la cabecera EBML es `webm`.
fn matroska(bytes: &[u8]) -> Option<FileKind> {
    if !bytes.starts_with(b"\x1a\x45\xdf\xa3") {
        return None;
    }
    let header = &bytes[..bytes.len().min(64)];
    Some(if contains(header, b"webm") {
        FileKind::Webm
    } else {
        FileKind::Mkv
    })
}

/// SVG es texto: su elemento raíz es `<svg`, tras el BOM y un prólogo de
/// espacios, la declaración XML, comentarios y un `<!DOCTYPE svg …>`. Un
/// HTML con un SVG adentro no lo es.
fn svg(bytes: &[u8]) -> Option<FileKind> {
    let head = &bytes[..bytes.len().min(SVG_PROBE)];
    let mut rest = head.strip_prefix(b"\xef\xbb\xbf").unwrap_or(head);
    loop {
        rest = rest.trim_ascii_start();
        rest = if rest.starts_with(b"<?xml") {
            after(rest, b"?>")?
        } else if rest.starts_with(b"<!--") {
            after(rest, b"-->")?
        } else if starts_with_ignore_case(rest, b"<!doctype svg") {
            after(rest, b">")?
        } else {
            break;
        };
    }
    let root = rest.strip_prefix(b"<svg")?;
    matches!(
        root.first(),
        Some(b'>' | b'/' | b' ' | b'\t' | b'\n' | b'\r')
    )
    .then_some(FileKind::Svg)
}

/// Lo que sigue a la primera aparición de `end` en `bytes`.
fn after<'b>(bytes: &'b [u8], end: &[u8]) -> Option<&'b [u8]> {
    let at = bytes.windows(end.len()).position(|window| window == end)?;
    Some(&bytes[at + end.len()..])
}

fn starts_with_ignore_case(bytes: &[u8], prefix: &[u8]) -> bool {
    bytes
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::png(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR", Some(FileKind::Png))]
    #[case::jpeg(b"\xff\xd8\xff\xe0\0\x10JFIF", Some(FileKind::Jpeg))]
    #[case::gif(b"GIF89a\x01\0", Some(FileKind::Gif))]
    #[case::webp(b"RIFF\x24\0\0\0WEBPVP8 ", Some(FileKind::Webp))]
    #[case::wav(b"RIFF\x24\0\0\0WAVEfmt ", Some(FileKind::Wav))]
    #[case::riff_desconocido(b"RIFF\x24\0\0\0XXXX", None)]
    #[case::bmp(b"BM\x36\0\0\0\0\0\0\0\x36\0", Some(FileKind::Bmp))]
    #[case::bm_que_no_es_bmp(b"BMW es una marca", None)]
    #[case::tiff(b"II*\0\x08\0\0\0", Some(FileKind::Tiff))]
    #[case::tiff_big_endian(b"MM\0*\0\0\0\x08", Some(FileKind::Tiff))]
    #[case::pdf(b"%PDF-1.7\n", Some(FileKind::Pdf))]
    #[case::avif(b"\0\0\0\x1cftypavif\0\0\0\0avifmif1miaf", Some(FileKind::Avif))]
    #[case::avif_por_compatible(b"\0\0\0\x18ftypmif1\0\0\0\0avifmiaf", Some(FileKind::Avif))]
    #[case::heic(b"\0\0\0\x18ftypheic\0\0\0\0mif1heic", Some(FileKind::Heic))]
    #[case::mp4(b"\0\0\0\x18ftypisom\0\0\x02\0isomiso2", Some(FileKind::Mp4))]
    #[case::mov(b"\0\0\0\x14ftypqt  \0\0\x02\0qt  ", Some(FileKind::Mov))]
    #[case::m4a(b"\0\0\0\x18ftypM4A \0\0\x02\0M4A mp42", Some(FileKind::M4a))]
    #[case::mp4_por_marca(b"\0\0\0\x18ftypmp42\0\0\0\0mp42isom", Some(FileKind::Mp4))]
    #[case::tres_gp(b"\0\0\0\x18ftyp3gp4\0\0\x02\0isom3gp4", None)]
    #[case::m4v(b"\0\0\0\x18ftypM4V \0\0\x02\0M4V mp42", None)]
    #[case::cr3(b"\0\0\0\x18ftypcrx \0\0\0\x01crx isom", None)]
    #[case::webm(
        b"\x1a\x45\xdf\xa3\x9f\x42\x86\x81\x01\x42\x82\x84webm",
        Some(FileKind::Webm)
    )]
    #[case::mkv(b"\x1a\x45\xdf\xa3\x9f\x42\x82\x88matroska", Some(FileKind::Mkv))]
    #[case::mp3(b"ID3\x04\0\0\0\0\0\0", Some(FileKind::Mp3))]
    #[case::ogg(b"OggS\0\x02", Some(FileKind::Ogg))]
    #[case::flac(b"fLaC\0\0\0\x22", Some(FileKind::Flac))]
    #[case::svg(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>", Some(FileKind::Svg))]
    #[case::svg_con_xml(b"\xef\xbb\xbf\n<?xml version=\"1.0\"?>\n<svg/>", Some(FileKind::Svg))]
    #[case::svg_con_comentario_y_doctype(
        b"<!-- hecho a mano --><!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\">\n<svg\nwidth=\"1\"/>",
        Some(FileKind::Svg)
    )]
    #[case::xml_que_no_es_svg(b"<?xml version=\"1.0\"?><feed/>", None)]
    #[case::html_con_un_svg(
        b"<!DOCTYPE html><html><head><link rel=\"icon\" href=\"data:image/svg+xml,<svg/>\">",
        None
    )]
    #[case::xml_con_un_svg_adentro(b"<?xml version=\"1.0\"?><mxfile><svg/></mxfile>", None)]
    #[case::otra_etiqueta_que_empieza_igual(b"<svgx/>", None)]
    #[case::prologo_sin_cerrar(b"<?xml version=\"1.0\"", None)]
    #[case::texto(b"hola", None)]
    #[case::zip(b"PK\x03\x04\x14\0", None)]
    #[case::vacio(b"", None)]
    fn detecta_por_contenido(#[case] bytes: &[u8], #[case] expected: Option<FileKind>) {
        assert_eq!(FileKind::detect(bytes), expected);
    }

    #[test]
    fn una_caja_ftyp_truncada_no_lee_fuera() {
        assert_eq!(
            FileKind::detect(b"\xff\xff\xff\xffftypavif"),
            Some(FileKind::Avif)
        );
        assert_eq!(FileKind::detect(b"\0\0\0\x18ftyp"), None);
    }
}
