"""Read the existing compositor XKB rules; no input capture or device access."""
import ctypes as C
import ctypes.util
import json
import sys


def describe(config):
    # Custom virtual-device keymaps may have no reconstructible rule names.
    # Do not silently compile the host's default US map and call it active.
    if not isinstance(config.get("layout"), str) or not config["layout"].strip():
        raise RuntimeError("Native XKB rules unavailable")
    lib = C.CDLL(ctypes.util.find_library("xkbcommon") or "libxkbcommon.so.0")
    class Names(C.Structure):
        _fields_ = [(key, C.c_char_p) for key in ("rules", "model", "layout", "variant", "options")]
    def fn(name, result, *args):
        value = getattr(lib, name); value.restype = result; value.argtypes = list(args)
        return value
    context_new = fn("xkb_context_new", C.c_void_p, C.c_int)
    context_free = fn("xkb_context_unref", None, C.c_void_p)
    keymap_new = fn("xkb_keymap_new_from_names", C.c_void_p, C.c_void_p, C.POINTER(Names), C.c_int)
    keymap_free = fn("xkb_keymap_unref", None, C.c_void_p)
    minimum = fn("xkb_keymap_min_keycode", C.c_uint, C.c_void_p)
    maximum = fn("xkb_keymap_max_keycode", C.c_uint, C.c_void_p)
    layouts = fn("xkb_keymap_num_layouts_for_key", C.c_uint, C.c_void_p, C.c_uint)
    levels = fn("xkb_keymap_num_levels_for_key", C.c_uint, C.c_void_p, C.c_uint, C.c_uint)
    symbols = fn("xkb_keymap_key_get_syms_by_level", C.c_int, C.c_void_p, C.c_uint, C.c_uint, C.c_uint, C.POINTER(C.POINTER(C.c_uint)))
    name = fn("xkb_keysym_get_name", C.c_int, C.c_uint, C.c_char_p, C.c_size_t)
    unicode = fn("xkb_keysym_to_utf32", C.c_uint, C.c_uint)
    context = context_new(0)
    if not context:
        raise RuntimeError("XKB context unavailable")
    keymap = None
    try:
        names = Names(*(str(config.get(key, "")).encode() or None for key, _ in Names._fields_))
        keymap = keymap_new(context, C.byref(names), 0)
        if not keymap:
            raise RuntimeError("Cannot read the active XKB rules")
        codes, labels = {}, {}
        # Include configured layouts/levels: report possible collisions, not just
        # the characters visible at the instant the preferences panel opens.
        for code in range(minimum(keymap), min(maximum(keymap), 775) + 1):
            for layout in range(layouts(keymap, code)):
                for level in range(levels(keymap, code, layout)):
                    syms = C.POINTER(C.c_uint)()
                    count = symbols(keymap, code, layout, level, C.byref(syms))
                    for index in range(count):
                        buffer = C.create_string_buffer(128)
                        if name(syms[index], buffer, len(buffer)) <= 0:
                            continue
                        symbol = buffer.value.decode()
                        values = codes.setdefault(symbol.lower(), [])
                        if code not in values:
                            values.append(code)
                        scalar = unicode(syms[index])
                        char = chr(scalar) if 0 < scalar <= 0x10FFFF else ""
                        if char.isprintable() and not char.isspace():
                            values = codes.setdefault(char.lower(), [])
                            if code not in values:
                                values.append(code)
                        if layout == config.get("active_layout_index", 0) and level == 0:
                            labels[str(code - 8)] = char if char.isprintable() and not char.isspace() else symbol
        return {"codes": codes, "labels": labels, "layout": config.get("active_keymap", config.get("layout", ""))}
    finally:
        if keymap:
            keymap_free(keymap)
        context_free(context)


if __name__ == "__main__":
    try:
        payload = sys.stdin.buffer.read(65537)
        if len(payload) > 65536:
            raise ValueError("XKB request too large")
        print(json.dumps(describe(json.loads(payload))))
    except Exception:
        # No configuration contents or subprocess arguments in errors.
        print(json.dumps({"error": "Native XKB keymap unavailable"}))
        sys.exit(1)
