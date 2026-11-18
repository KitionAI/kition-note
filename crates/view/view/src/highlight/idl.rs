//! Interface languages, as their CodeMirror 5 modes color them, each ported token for token: IDL
//! (the data language) and Web IDL (see each function).

use super::Kind;
use super::scan::{self, Out, Stream};

/// IDL, as CodeMirror 5's idl mode colors it: keywords and its routines' names builtins (any
/// case), other names variables, numbers, strings (doubled quotes inside), symbols operators,
/// `;` comments.
pub fn idl(text: &str, out: &mut Out) {
    scan::run(text, out, |stream| {
        if stream.eat_space() {
            return None;
        }
        if stream.take(";") {
            stream.skip_to_end();
            return Some(Kind::Comment);
        }
        if let Some(end) = idl_number(stream) {
            stream.pos = end;
            return Some(Kind::Number);
        }
        let quote = stream.peek();
        if quote == b'"' || quote == b'\'' {
            // `"([^"]|(""))*"`: to the last quote of the run the pattern takes.
            let mut at = stream.pos + 1;
            let mut end = None;
            while at < stream.bytes.len() {
                if stream.byte(at) == quote {
                    end = Some(at + 1);
                    if stream.byte(at + 1) != quote {
                        break;
                    }
                    at += 2;
                    continue;
                }
                at += 1;
            }
            if let Some(end) = end {
                stream.pos = end;
                return Some(Kind::String);
            }
        }
        if stream.take_word_of(IDL_KEYWORDS, true) {
            return Some(Kind::Keyword);
        }
        if stream.take_word_of(IDL_BUILTINS, true) {
            return Some(Kind::Property);
        }
        let here = stream.peek();
        if here.is_ascii_alphabetic() || here == b'_' || here >= 0x80 {
            stream.eat_while(|byte| scan::js_word(byte) || byte >= 0x80);
            return Some(Kind::Variable);
        }
        stream.next();
        b"+-*&=<>/@#~$".contains(&here).then_some(Kind::Operator)
    });
}

/// `[+-]?0x…`, `[+-]?\d*\.\d+([EeDd][+-]?\d+)?`, `[+-]?\d+([EeDd][+-]?\d+)?`: where it ends.
fn idl_number(stream: &Stream) -> Option<usize> {
    let at = stream.pos;
    let sign = if matches!(stream.byte(at), b'+' | b'-') { at + 1 } else { at };
    let run = |from: usize, ok: fn(u8) -> bool| (from..stream.bytes.len()).find(|end| !ok(stream.byte(*end))).unwrap_or(stream.bytes.len().max(from));
    let exponent = |from: usize| {
        if !b"EeDd".contains(&stream.byte(from)) {
            return from;
        }
        let digits = if matches!(stream.byte(from + 1), b'+' | b'-') { from + 2 } else { from + 1 };
        let end = run(digits, |byte| byte.is_ascii_digit());
        if end > digits { end } else { from }
    };
    if stream.byte(sign) == b'0' && stream.byte(sign + 1) == b'x' {
        let end = run(sign + 2, |byte| byte.is_ascii_hexdigit());
        if end > sign + 2 {
            return Some(end);
        }
    }
    let whole = run(sign, |byte| byte.is_ascii_digit());
    if stream.byte(whole) == b'.' {
        let fraction = run(whole + 1, |byte| byte.is_ascii_digit());
        if fraction > whole + 1 {
            return Some(exponent(fraction));
        }
    }
    (whole > sign).then(|| exponent(whole))
}

#[derive(Clone, Debug, Default)]
pub struct WebIdl {
    comment: bool,
    last: String,
    start_def: bool,
    end_def: bool,
}

/// `_?[A-Za-z][0-9A-Z_a-z-]*` at the position: where it ends.
fn webidl_name(stream: &Stream) -> Option<usize> {
    let at = stream.pos;
    let first = if stream.byte(at) == b'_' { at + 1 } else { at };
    if !stream.byte(first).is_ascii_alphabetic() {
        return None;
    }
    Some((first + 1..stream.bytes.len()).find(|end| !(scan::js_word(stream.byte(*end)) || stream.byte(*end) == b'-')).unwrap_or(stream.bytes.len()))
}

/// What `^\s*(.+?)\b` takes after `at`: the shortest text after the spaces that ends at a word
/// boundary (taking fewer spaces when none does).
fn next_word<'a>(stream: &Stream<'a>, at: usize) -> Option<&'a str> {
    let len = stream.bytes.len();
    let boundary = |index: usize| {
        let before = index.checked_sub(1).is_some_and(|index| scan::js_word(stream.byte(index)));
        let after = index < len && scan::js_word(stream.byte(index));
        before != after
    };
    let spaces = (at..len).find(|index| !stream.byte(*index).is_ascii_whitespace()).unwrap_or(len);
    (at..=spaces).rev().find_map(|from| {
        (from + 1..=len).find(|end| boundary(*end)).and_then(|end| stream.bytes.get(from..end)).and_then(|text| std::str::from_utf8(text).ok())
    })
}

/// Web IDL, as CodeMirror 5's webidl mode colors it: keywords, types (builtins after `:` or
/// `implements` and before `implements` or `=`, else variable-3), extended attributes builtins,
/// `true`, `null`, … atoms (plain), a defined name plain, other names variables, numbers, strings,
/// `//` and `/* */` comments, `:<=>?` operators.
pub fn webidl(text: &str, state: &mut WebIdl, out: &mut Out) {
    scan::run(text, out, |stream| {
        let style = webidl_token(stream, state);
        if let Some(kind) = style {
            let current = stream.current().to_string();
            if kind == Some(Kind::Keyword) {
                state.start_def = ["callback", "dictionary", "enum", "interface"].iter().any(|word| word_starts(&current, word));
                state.end_def = state.end_def || word_starts(&current, "typedef");
            } else {
                state.start_def = false;
            }
            state.last = current;
        }
        style.flatten()
    });
}

/// `^((word))\b` tested on a token.
fn word_starts(text: &str, word: &str) -> bool {
    text.strip_prefix(word).is_some_and(|rest| !rest.bytes().next().is_some_and(scan::js_word))
}

/// The next token's style: `None` for none, `Some(None)` for one without a color of its own.
fn webidl_token(stream: &mut Stream, state: &mut WebIdl) -> Option<Option<Kind>> {
    if stream.eat_space() {
        return None;
    }
    let rest_find = |stream: &Stream, needle: &[u8]| stream.rest().windows(needle.len()).position(|window| window == needle);
    if state.comment {
        match rest_find(stream, b"*/") {
            Some(at) => {
                stream.pos += at + 2;
                state.comment = false;
            }
            None => stream.skip_to_end(),
        }
        return Some(Some(Kind::Comment));
    }
    if stream.take("//") {
        stream.skip_to_end();
        return Some(Some(Kind::Comment));
    }
    if stream.take("/*") {
        match rest_find(stream, b"*/") {
            Some(at) => stream.pos += at + 2,
            None => {
                state.comment = true;
                stream.skip_to_end();
            }
        }
        return Some(Some(Kind::Comment));
    }
    let sign = if stream.peek() == b'-' { stream.pos + 1 } else { stream.pos };
    if (stream.byte(sign).is_ascii_digit() || stream.byte(sign) == b'.')
        && let Some(end) = webidl_integer(stream, sign).or_else(|| webidl_float(stream, sign))
    {
        stream.pos = end;
        return Some(Some(Kind::Number));
    }
    if stream.peek() == b'"'
        && let Some(close) = stream.bytes.get(stream.pos + 1..).and_then(|rest| rest.iter().position(|byte| *byte == b'"'))
    {
        stream.pos += close + 2;
        return Some(Some(Kind::String));
    }
    if state.start_def
        && let Some(end) = webidl_name(stream)
    {
        stream.pos = end;
        return Some(None);
    }
    if state.end_def
        && let Some(end) = webidl_name(stream)
    {
        let after = (end..stream.bytes.len()).find(|at| !stream.byte(*at).is_ascii_whitespace());
        if after.is_some_and(|at| stream.byte(at) == b';') {
            stream.pos = end;
            state.end_def = false;
            return Some(None);
        }
    }
    if stream.take_word_of(WEBIDL_KEYWORDS, false) {
        return Some(Some(Kind::Keyword));
    }
    if stream.take_word_of(WEBIDL_TYPES, false) {
        let next = next_word(stream, stream.pos);
        let builtin = state.last == ":" || state.last == "implements" || next == Some("implements") || next == Some("=");
        return Some(Some(if builtin { Kind::Property } else { Kind::Variable }));
    }
    if stream.take_word_of(WEBIDL_ATTRIBUTES, false) {
        return Some(Some(Kind::Property));
    }
    if stream.take_word_of(WEBIDL_ATOMS, false) {
        return Some(None);
    }
    if let Some(end) = webidl_name(stream) {
        stream.pos = end;
        return Some(Some(Kind::Variable));
    }
    if b":<=>?".contains(&stream.peek()) {
        stream.next();
        return Some(Some(Kind::Operator));
    }
    stream.next();
    None
}

/// `-?([1-9][0-9]*|0[Xx][0-9A-Fa-f]+|0[0-7]*)` (the sign before `at`).
fn webidl_integer(stream: &Stream, at: usize) -> Option<usize> {
    let run = |from: usize, ok: fn(u8) -> bool| (from..stream.bytes.len()).find(|end| !ok(stream.byte(*end))).unwrap_or(stream.bytes.len().max(from));
    match stream.byte(at) {
        b'1'..=b'9' => Some(run(at + 1, |byte| byte.is_ascii_digit())),
        b'0' if matches!(stream.byte(at + 1), b'x' | b'X') && stream.byte(at + 2).is_ascii_hexdigit() => {
            Some(run(at + 2, |byte| byte.is_ascii_hexdigit()))
        }
        b'0' => Some(run(at + 1, |byte| (b'0'..=b'7').contains(&byte))),
        _ => None,
    }
}

/// `-?(([0-9]+\.[0-9]*|[0-9]*\.[0-9]+)([Ee][+-]?[0-9]+)?|[0-9]+[Ee][+-]?[0-9]+)`.
fn webidl_float(stream: &Stream, at: usize) -> Option<usize> {
    let run = |from: usize| (from..stream.bytes.len()).find(|end| !stream.byte(*end).is_ascii_digit()).unwrap_or(stream.bytes.len().max(from));
    let exponent = |from: usize| {
        if !matches!(stream.byte(from), b'e' | b'E') {
            return None;
        }
        let digits = if matches!(stream.byte(from + 1), b'+' | b'-') { from + 2 } else { from + 1 };
        let end = run(digits);
        (end > digits).then_some(end)
    };
    let whole = run(at);
    if stream.byte(whole) == b'.' {
        let fraction = run(whole + 1);
        if whole > at || fraction > whole + 1 {
            return Some(exponent(fraction).unwrap_or(fraction));
        }
    }
    if whole > at { exponent(whole) } else { None }
}

/// IDL's routines (the mode's list, `|` between them).
const IDL_BUILTINS: &str = "a_correlate|abs|acos|adapt_hist_equal|alog|alog2|alog10|amoeba|annotate|app_user_dir|app_user_dir_query|arg_present|\
array_equal|array_indices|arrow|ascii_template|asin|assoc|atan|axis|axis|bandpass_filter|bandreject_filter|barplot|bar_plot|\
beseli|beselj|beselk|besely|beta|biginteger|bilinear|bin_date|binary_template|bindgen|binomial|bit_ffs|bit_population|\
blas_axpy|blk_con|boolarr|boolean|boxplot|box_cursor|breakpoint|broyden|bubbleplot|butterworth|bytarr|byte|byteorder|bytscl|\
c_correlate|calendar|caldat|call_external|call_function|call_method|call_procedure|canny|catch|cd|cdf|ceil|chebyshev|\
check_math|chisqr_cvf|chisqr_pdf|choldc|cholsol|cindgen|cir_3pnt|clipboard|close|clust_wts|cluster|cluster_tree|cmyk_convert|\
code_coverage|color_convert|color_exchange|color_quan|color_range_map|colorbar|colorize_sample|colormap_applicable|\
colormap_gradient|colormap_rotation|colortable|comfit|command_line_args|common|compile_opt|complex|complexarr|complexround|\
compute_mesh_normals|cond|congrid|conj|constrained_min|contour|contour|convert_coord|convol|convol_fft|coord2to3|copy_lun|\
correlate|cos|cosh|cpu|cramer|createboxplotdata|create_cursor|create_struct|create_view|crossp|crvlength|ct_luminance|\
cti_test|cursor|curvefit|cv_coord|cvttobm|cw_animate|cw_animate_getp|cw_animate_load|cw_animate_run|cw_arcball|cw_bgroup|\
cw_clr_index|cw_colorsel|cw_defroi|cw_field|cw_filesel|cw_form|cw_fslider|cw_light_editor|cw_light_editor_get|\
cw_light_editor_set|cw_orient|cw_palette_editor|cw_palette_editor_get|cw_palette_editor_set|cw_pdmenu|cw_rgbslider|cw_tmpl|\
cw_zoom|db_exists|dblarr|dcindgen|dcomplex|dcomplexarr|define_key|define_msgblk|define_msgblk_from_file|defroi|defsysv|\
delvar|dendro_plot|dendrogram|deriv|derivsig|determ|device|dfpmin|diag_matrix|dialog_dbconnect|dialog_message|\
dialog_pickfile|dialog_printersetup|dialog_printjob|dialog_read_image|dialog_write_image|dictionary|digital_filter|dilate|\
dindgen|dissolve|dist|distance_measure|dlm_load|dlm_register|doc_library|double|draw_roi|edge_dog|efont|eigenql|eigenvec|\
ellipse|elmhes|emboss|empty|enable_sysrtn|eof|eos|erase|erf|erfc|erfcx|erode|errorplot|errplot|estimator_filter|execute|exit|\
exp|expand|expand_path|expint|extract|extract_slice|f_cvf|f_pdf|factorial|fft|file_basename|file_chmod|file_copy|file_delete|\
file_dirname|file_expand_path|file_gunzip|file_gzip|file_info|file_lines|file_link|file_mkdir|file_move|file_poll_input|\
file_readlink|file_same|file_search|file_tar|file_test|file_untar|file_unzip|file_which|file_zip|filepath|findgen|finite|fix|\
flick|float|floor|flow3|fltarr|flush|format_axis_values|forward_function|free_lun|fstat|fulstr|funct|function|fv_test|\
fx_root|fz_roots|gamma|gamma_ct|gauss_cvf|gauss_pdf|gauss_smooth|gauss2dfit|gaussfit|gaussian_function|gaussint|\
get_drive_list|get_dxf_objects|get_kbrd|get_login_info|get_lun|get_screen_size|getenv|getwindows|greg2jul|grib|grid_input|\
grid_tps|grid3|griddata|gs_iter|h_eq_ct|h_eq_int|hanning|hash|hdf|hdf5|heap_free|heap_gc|heap_nosave|heap_refcount|heap_save|\
help|hilbert|hist_2d|hist_equal|histogram|hls|hough|hqr|hsv|i18n_multibytetoutf8|i18n_multibytetowidechar|\
i18n_utf8tomultibyte|i18n_widechartomultibyte|ibeta|icontour|iconvertcoord|idelete|identity|idl_base64|idl_container|\
idl_validname|idlexbr_assistant|idlitsys_createtool|idlunit|iellipse|igamma|igetcurrent|igetdata|igetid|igetproperty|iimage|\
image|image_cont|image_statistics|image_threshold|imaginary|imap|indgen|int_2d|int_3d|int_tabulated|intarr|interpol|\
interpolate|interval_volume|invert|ioctl|iopen|ir_filter|iplot|ipolygon|ipolyline|iputdata|iregister|ireset|iresolve|irotate|\
isa|isave|iscale|isetcurrent|isetproperty|ishft|isocontour|isosurface|isurface|itext|itranslate|ivector|ivolume|izoom|\
journal|json_parse|json_serialize|jul2greg|julday|keyword_set|krig2d|kurtosis|kw_test|l64indgen|la_choldc|la_cholmprove|\
la_cholsol|la_determ|la_eigenproblem|la_eigenql|la_eigenvec|la_elmhes|la_gm_linear_model|la_hqr|la_invert|\
la_least_square_equality|la_least_squares|la_linear_equation|la_ludc|la_lumprove|la_lusol|la_svd|la_tridc|la_trimprove|\
la_triql|la_trired|la_trisol|label_date|label_region|ladfit|laguerre|lambda|lambdap|lambertw|laplacian|least_squares_filter|\
leefilt|legend|legendre|linbcg|lindgen|linfit|linkimage|list|ll_arc_distance|lmfit|lmgr|lngamma|lnp_test|loadct|locale_get|\
logical_and|logical_or|logical_true|lon64arr|lonarr|long|long64|lsode|lu_complex|ludc|lumprove|lusol|m_correlate|machar|\
make_array|make_dll|make_rt|map|mapcontinents|mapgrid|map_2points|map_continents|map_grid|map_image|map_patch|\
map_proj_forward|map_proj_image|map_proj_info|map_proj_init|map_proj_inverse|map_set|matrix_multiply|matrix_power|max|\
md_test|mean|meanabsdev|mean_filter|median|memory|mesh_clip|mesh_decimate|mesh_issolid|mesh_merge|mesh_numtriangles|mesh_obj|\
mesh_smooth|mesh_surfacearea|mesh_validate|mesh_volume|message|min|min_curve_surf|mk_html_help|modifyct|moment|morph_close|\
morph_distance|morph_gradient|morph_hitormiss|morph_open|morph_thin|morph_tophat|multi|n_elements|n_params|n_tags|ncdf|\
newton|noise_hurl|noise_pick|noise_scatter|noise_slur|norm|obj_class|obj_destroy|obj_hasmethod|obj_isa|obj_new|obj_valid|\
objarr|on_error|on_ioerror|online_help|openr|openu|openw|oplot|oploterr|orderedhash|p_correlate|parse_url|particle_trace|\
path_cache|path_sep|pcomp|plot|plot3d|plot|plot_3dbox|plot_field|ploterr|plots|polar_contour|polar_surface|polyfill|\
polyshade|pnt_line|point_lun|polarplot|poly|poly_2d|poly_area|poly_fit|polyfillv|polygon|polyline|polywarp|popd|powell|\
pref_commit|pref_get|pref_set|prewitt|primes|print|printf|printd|pro|product|profile|profiler|profiles|project_vol|\
ps_show_fonts|psafm|pseudo|ptr_free|ptr_new|ptr_valid|ptrarr|pushd|qgrid3|qhull|qromb|qromo|qsimp|query_*|query_ascii|\
query_bmp|query_csv|query_dicom|query_gif|query_image|query_jpeg|query_jpeg2000|query_mrsid|query_pict|query_png|query_ppm|\
query_srf|query_tiff|query_video|query_wav|r_correlate|r_test|radon|randomn|randomu|ranks|rdpix|read|readf|read_ascii|\
read_binary|read_bmp|read_csv|read_dicom|read_gif|read_image|read_interfile|read_jpeg|read_jpeg2000|read_mrsid|read_pict|\
read_png|read_ppm|read_spr|read_srf|read_sylk|read_tiff|read_video|read_wav|read_wave|read_x11_bitmap|read_xwd|reads|readu|\
real_part|rebin|recall_commands|recon3|reduce_colors|reform|region_grow|register_cursor|regress|replicate|replicate_inplace|\
resolve_all|resolve_routine|restore|retall|return|reverse|rk4|roberts|rot|rotate|round|routine_filepath|routine_info|rs_test|\
s_test|save|savgol|scale3|scale3d|scatterplot|scatterplot3d|scope_level|scope_traceback|scope_varfetch|scope_varname|\
search2d|search3d|sem_create|sem_delete|sem_lock|sem_release|set_plot|set_shading|setenv|sfit|shade_surf|shade_surf_irr|\
shade_volume|shift|shift_diff|shmdebug|shmmap|shmunmap|shmvar|show3|showfont|signum|simplex|sin|sindgen|sinh|size|skewness|\
skip_lun|slicer3|slide_image|smooth|sobel|socket|sort|spawn|sph_4pnt|sph_scat|spher_harm|spl_init|spl_interp|spline|spline_p|\
sprsab|sprsax|sprsin|sprstp|sqrt|standardize|stddev|stop|strarr|strcmp|strcompress|streamline|streamline|stregex|stretch|\
string|strjoin|strlen|strlowcase|strmatch|strmessage|strmid|strpos|strput|strsplit|strtrim|struct_assign|struct_hide|\
strupcase|surface|surface|surfr|svdc|svdfit|svsol|swap_endian|swap_endian_inplace|symbol|systime|t_cvf|t_pdf|t3d|tag_names|\
tan|tanh|tek_color|temporary|terminal_size|tetra_clip|tetra_surface|tetra_volume|text|thin|thread|threed|tic|time_test2|\
timegen|timer|timestamp|timestamptovalues|tm_test|toc|total|trace|transpose|tri_surf|triangulate|trigrid|triql|trired|trisol|\
truncate_lun|ts_coef|ts_diff|ts_fcast|ts_smooth|tv|tvcrs|tvlct|tvrd|tvscl|typename|uindgen|uint|uintarr|ul64indgen|ulindgen|\
ulon64arr|ulonarr|ulong|ulong64|uniq|unsharp_mask|usersym|value_locate|variance|vector|vector_field|vel|velovect|vert_t3d|\
voigt|volume|voronoi|voxel_proj|wait|warp_tri|watershed|wdelete|wf_draw|where|widget_base|widget_button|widget_combobox|\
widget_control|widget_displaycontextmenu|widget_draw|widget_droplist|widget_event|widget_info|widget_label|widget_list|\
widget_propertysheet|widget_slider|widget_tab|widget_table|widget_text|widget_tree|widget_tree_move|widget_window|\
wiener_filter|window|window|write_bmp|write_csv|write_gif|write_image|write_jpeg|write_jpeg2000|write_nrif|write_pict|\
write_png|write_ppm|write_spr|write_srf|write_sylk|write_tiff|write_video|write_wav|write_wave|writeu|wset|wshow|wtn|\
wv_applet|wv_cwt|wv_cw_wavelet|wv_denoise|wv_dwt|wv_fn_coiflet|wv_fn_daubechies|wv_fn_gaussian|wv_fn_haar|wv_fn_morlet|\
wv_fn_paul|wv_fn_symlet|wv_import_data|wv_import_wavelet|wv_plot3d_wps|wv_plot_multires|wv_pwt|wv_tool_denoise|xbm_edit|\
xdisplayfile|xdxf|xfont|xinteranimate|xloadct|xmanager|xmng_tmpl|xmtool|xobjview|xobjview_rotate|xobjview_write_image|\
xpalette|xpcolor|xplot3d|xregistered|xroi|xsq_test|xsurface|xvaredit|xvolume|xvolume_rotate|xvolume_write_image|xyouts|\
zlib_compress|zlib_uncompress|zoom|zoom_24";

const IDL_KEYWORDS: &str = "begin|end|endcase|endfor|endwhile|endif|endrep|endforeach|break|case|continue|for|foreach|goto|if|then|else|repeat|until|\
switch|while|do|pro|function";

/// Extended attributes.
const WEBIDL_ATTRIBUTES: &str = "Clamp|Constructor|EnforceRange|Exposed|ImplicitThis|Global|PrimaryGlobal|LegacyArrayClass|LegacyUnenumerableNamedProperties|\
LenientThis|NamedConstructor|NewObject|NoInterfaceObject|OverrideBuiltins|PutForwards|Replaceable|SameObject|\
TreatNonObjectAsNull|TreatNullAs|EmptyString|Unforgeable|Unscopeable";

const WEBIDL_TYPES: &str = "unsigned|short|long|unrestricted|float|double|boolean|byte|octet|Promise|ArrayBuffer|DataView|Int8Array|Int16Array|\
Int32Array|Uint8Array|Uint16Array|Uint32Array|Uint8ClampedArray|Float32Array|Float64Array|ByteString|DOMString|USVString|\
sequence|object|RegExp|Error|DOMException|FrozenArray|any|void";

const WEBIDL_KEYWORDS: &str = "attribute|callback|const|deleter|dictionary|enum|getter|implements|inherit|interface|iterable|legacycaller|maplike|partial|\
required|serializer|setlike|setter|static|stringifier|typedef|optional|readonly|or";

const WEBIDL_ATOMS: &str = "true|false|Infinity|NaN|null";
