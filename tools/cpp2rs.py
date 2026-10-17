import re, sys

def camel_to_snake(n):
    s = re.sub(r'(?<=[a-z0-9])([A-Z])', r'_\1', n)
    return s.lower()

def camel_to_upper(n):
    return camel_to_snake(n).upper()

def extract_function(src, header_regex):
    m = re.search(header_regex, src)
    if not m:
        return None
    i = src.index('{', m.end())
    depth = 0
    j = i
    while True:
        c = src[j]
        if c == '{': depth += 1
        elif c == '}':
            depth -= 1
            if depth == 0: break
        j += 1
    return src[i+1:j]

def split_top(s, sep=','):
    parts, depth, cur = [], 0, ''
    for ch in s:
        if ch in '([{<' and not (ch == '<' ): depth += 1
        if ch in ')]}' : depth -= 1
        if ch == sep and depth == 0:
            parts.append(cur); cur = ''
        else:
            cur += ch
    parts.append(cur)
    return parts

def find_close(s, i):
    # s[i] == '(' ; return index of matching ')'
    d = 0
    for j in range(i, len(s)):
        if s[j] == '(': d += 1
        elif s[j] == ')':
            d -= 1
            if d == 0: return j
    raise ValueError('unbalanced: ' + s[i:i+80])

HELPERS = {  # C++ name -> (rust method, drop-args-by-name)
}

DROP = {'level', 'chunkBB'}

class T:
    def __init__(self, fields=(), consts=None, tile_alias=None, known_tiles=None, extra_methods=None, selectors=None):
        self.fields = set(fields)
        self.consts = consts or {}
        self.tile_alias = tile_alias or {}
        self.known_tiles = known_tiles
        self.unknown_tiles = set()
        self.selectors = selectors or {}
        self.extra = extra_methods or {}
        self.tmp = 0

    def tile(self, name):
        if name in self.tile_alias: n = self.tile_alias[name]
        else: n = camel_to_upper(name)
        if self.known_tiles is not None and n not in self.known_tiles:
            self.unknown_tiles.add((name, n))
        return 'ti::' + n

    def expr(self, e):
        e = e.strip()
        # comments
        e = re.sub(r'//.*', '', e)
        # float literals
        e = re.sub(r'(?<![\w.])(\d*\.\d+)f\b', lambda m: ('0' + m.group(1) if m.group(1).startswith('.') else m.group(1)) + 'f32', e)
        e = re.sub(r'(?<![\w.])(\d+)\.f\b', r'\1.0f32', e)
        e = re.sub(r'(?<![\w.])(\d+)f\b', r'\1.0f32', e)
        # tiles
        e = re.sub(r'Tile::(\w+)_Id\b', lambda m: self.tile(m.group(1)), e)
        e = re.sub(r'Tile::(\w+)->id\b', lambda m: self.tile(m.group(1)), e)
        e = re.sub(r'\bTile::tiles\b', 'TILES', e)
        # random
        e = e.replace('random->nextInt(', 'c.rnd.next_int_bounded(')
        e = e.replace('random->nextBoolean()', 'c.rnd.next_boolean()')
        e = e.replace('random->nextFloat()', 'c.rnd.next_float()')
        e = e.replace('random->nextDouble()', 'c.rnd.next_double()')
        e = re.sub(r'Mth::nextInt\(random,', 'mc_core::mth::next_int(c.rnd,', e)
        e = re.sub(r'Math::_max\(', 'std::cmp::max(', e)
        e = re.sub(r'Math::_min\(', 'std::cmp::min(', e)
        e = re.sub(r'Math::abs\(', '(', e)
        e = e.replace('chunkBB->isInside(', 'c.cb.is_inside(')
        e = re.sub(r'boundingBox->(\w+)\(\)', lambda m: 'c.f.bb.' + camel_to_snake(m.group(1)) + '()', e)
        e = re.sub(r'boundingBox->(x0|x1|y0|y1|z0|z1)\b', r'c.f.bb.\1', e)
        e = re.sub(r'\borientation\b', 'c.f.orientation', e)
        e = re.sub(r'\bDirection::NORTH\b', 'DIR_NORTH', e)
        e = re.sub(r'\bDirection::SOUTH\b', 'DIR_SOUTH', e)
        e = re.sub(r'\bDirection::EAST\b', 'DIR_EAST', e)
        e = re.sub(r'\bDirection::WEST\b', 'DIR_WEST', e)
        e = re.sub(r'\bFacing::(\w+)\b', lambda m: 'fc::' + m.group(1), e)
        for k, v in self.consts.items():
            e = re.sub(r'(?<![\w.])' + k + r'\b', v, e)
        for fld in self.fields:
            e = re.sub(r'(?<![\w.>:])' + fld + r'\b', 'self.' + camel_to_snake(fld), e)
        # method calls with level/chunkBB dropping
        e = self.calls(e)
        e = e.replace('nullptr', 'None')
        return e

    CALLS = ['placeBlock', 'getBlock', 'generateBox', 'generateMaybeBox', 'maybeGenerateBlock', 'generateAirColumnUp', 'fillColumnDown',
             'createDoor', 'createChest', 'getWorldX', 'getWorldY', 'getWorldZ', 'getOrientationData', 'spawnVillagers', 'generateUpperHalfSphere',
             'edgesLiquid', 'getAverageGroundHeight', 'createDispenser', 'generateSmallDoor', 'randomSmallDoor', 'generateAirBox', 'updateAverageGroundHeight']

    def calls(self, e):
        out = ''
        i = 0
        while i < len(e):
            m = re.compile(r'(?<![\w.>:])(' + '|'.join(self.CALLS) + r')\(').match(e, i)
            if m:
                name = m.group(1)
                op = m.end() - 1
                cl = find_close(e, op)
                args = [a.strip() for a in split_top(e[op+1:cl])]
                args = [self.calls(a) for a in args]
                args = [a for a in args if a not in DROP]
                out += self.call_to_rust(name, args)
                i = cl + 1
            else:
                out += e[i]
                i += 1
        return out

    def call_to_rust(self, name, args):
        if name == 'generateBox':
            # drop `random` arg for selector overload
            if len(args) >= 9 and ('random' in args or 'c.rnd' in args):
                args = [a for a in args if a not in ('random',)]
                sel = args[-1]
                sel = re.sub(r'\(BlockSelector\s*\*\)\s*', '', sel)
                return 'c.generate_box_sel(%s, %s)' % (', '.join(args[:-1]), self.selectors.get(sel, sel))
            if len(args) == 9: return 'c.generate_box(%s)' % ', '.join(args)
            if len(args) == 11: return 'c.generate_box_data(%s)' % ', '.join(args)
            raise ValueError('generateBox args %d: %s' % (len(args), args))
        if name == 'generateAirBox':
            return 'c.generate_box(%s, 0, 0, false)' % ', '.join(args)
        if name == 'updateAverageGroundHeight':
            return 'p.update_average_ground_height(c, %s)' % ', '.join(args)
        if name == 'generateMaybeBox':
            args = [a for a in args if a != 'random']
            return 'c.generate_maybe_box(%s)' % ', '.join(args)
        if name == 'maybeGenerateBlock':
            args = [a for a in args if a != 'random']
            return 'c.maybe_generate_block(%s)' % ', '.join(args)
        if name == 'createDoor':
            args = [a for a in args if a != 'random']
            return 'c.create_door(%s)' % ', '.join(args)
        if name == 'createChest':
            args = [a for a in args if a != 'random']
            return 'c.create_chest(%s)' % ', '.join(args)
        if name == 'createDispenser':
            args = [a for a in args if a != 'random']
            return 'c.create_dispenser(%s)' % ', '.join(args)
        if name == 'generateUpperHalfSphere':
            return 'c.generate_upper_half_sphere(%s)' % ', '.join(args)
        if name == 'getAverageGroundHeight':
            return 'c.get_average_ground_height()'
        if name == 'spawnVillagers':
            return 'self.spawn_villagers(c, %s)' % ', '.join(args)
        if name == 'edgesLiquid':
            return 'c.edges_liquid()'
        rn = {'getWorldX': 'world_x', 'getWorldY': 'world_y', 'getWorldZ': 'world_z'}.get(name, camel_to_snake(name))
        return 'c.%s(%s)' % (rn, ', '.join(args))

    # ---- statement level ----
    def stmt_lines(self, body):
        # remove comments, normalise
        lines = []
        for ln in body.split('\n'):
            ln = re.sub(r'//.*', '', ln).rstrip()
            if ln.strip(): lines.append(ln.strip())
        text = '\n'.join(lines)
        text = self.add_braces(text)
        out = []
        for ln in text.split('\n'):
            out.append(self.stmt(ln))
        return '\n'.join(out)

    def add_braces(self, text):
        # tokenise to statements: split by ';', '{', '}' while keeping parens intact
        toks = []
        cur = ''
        depth = 0
        for ch in text:
            if ch == '(': depth += 1
            if ch == ')': depth -= 1
            if depth == 0 and ch in ';{}':
                if ch == ';':
                    cur += ch
                    toks.append(cur.strip()); cur = ''
                else:
                    if cur.strip(): toks.append(cur.strip())
                    toks.append(ch); cur = ''
            else:
                cur += ch if ch != '\n' else ' '
        if cur.strip(): toks.append(cur.strip())
        # now insert braces for if/else/for/while without '{'
        out = []
        i = 0
        def parse_stmt(i):
            """parse one statement starting at toks[i]; return (list of toks, next index)"""
            t = toks[i]
            if t == '{':
                # block
                res = ['{']; i += 1
                while toks[i] != '}':
                    sub, i = parse_stmt(i)
                    res += sub
                res.append('}')
                return res, i + 1
            m = re.match(r'^(if|for|while)\b', t) or re.match(r'^(else if)\b', t) or re.match(r'^(else)$', t)
            if m:
                # header token (t) ends w/o ';' ; body follows
                head = t
                # handle "else" followed by "if (...)" inside same token e.g. "else if (...)"
                body, j = parse_stmt(i + 1)
                if body[0] != '{':
                    body = ['{'] + body + ['}']
                return [head] + body, j
            return [t], i + 1
        while i < len(toks):
            sub, i = parse_stmt(i)
            out += sub
        # join "} else" handled by stmt()
        return '\n'.join(out)

    def stmt(self, ln):
        ind = ''
        if ln in ('{', '}'):
            return ln
        m = re.match(r'^else if\s*\((.*)\)$', ln)
        if m: return '} else if %s ' % self.cond(m.group(1)) if False else 'ELSEIF ' + self.cond(m.group(1))
        if ln == 'else': return 'ELSE'
        m = re.match(r'^if\s*\((.*)\)$', ln)
        if m: return 'if %s' % self.cond(m.group(1))
        m = re.match(r'^while\s*\((.*)\)$', ln)
        if m: return 'while %s' % self.cond(m.group(1))
        m = re.match(r'^for\s*\((.*)\)$', ln)
        if m: return self.for_loop(m.group(1))
        m = re.match(r'^(int|bool|float|double)\s+(.*);$', ln)
        if m:
            ty = {'int': 'i32', 'bool': 'bool', 'float': 'f32', 'double': 'f64'}[m.group(1)]
            decls = split_top(m.group(2))
            res = []
            for d in decls:
                if '=' in d:
                    nm, val = d.split('=', 1)
                    res.append('let mut %s: %s = %s;' % (nm.strip(), ty, self.expr(val)))
                else:
                    res.append('let mut %s: %s = Default::default();' % (d.strip(), ty))
            return ' '.join(res)
        m = re.match(r'^return\s*(.*);$', ln)
        if m: return 'return %s;' % (self.expr(m.group(1)) if m.group(1) else '')
        m = re.match(r'^boundingBox->move\((.*)\);$', ln)
        if m: return 'c.f.bb.move_by(%s);' % self.expr(m.group(1))
        # assignment / call
        e = ln.rstrip(';')
        e = self.expr(e)
        e = re.sub(r'\+\+$', ' += 1', e)
        e = re.sub(r'--$', ' -= 1', e)
        return self.hoist(e + ';')

    RND = re.compile(r'(c\.rnd\.\w+\([^()]*\)|mc_core::mth::next_int\(c\.rnd,[^()]*\))')

    def hoist(self, st):
        calls = self.RND.findall(st)
        if not calls:
            return st
        rest = self.RND.sub('', st)
        if not re.search(r'\bc\.\w+\(', rest):
            return st
        pre = ''
        for k, cl in enumerate(calls):
            tmp = '_r%d' % (self.tmp)
            self.tmp += 1
            pre += 'let %s: i32 = %s; ' % (tmp, cl)
            st = st.replace(cl, tmp, 1)
        return pre + st

    def cond(self, s):
        s = self.expr(s)
        return s + ' '

    def for_loop(self, hdr):
        parts = split_top(hdr, ';')
        init, cond, step = [p.strip() for p in parts]
        m = re.match(r'^int\s+(\w+)\s*=\s*(.*)$', init)
        var, start = m.group(1), m.group(2)
        cm = re.match(r'^(\w+)\s*(<=|<|>=|>)\s*(.*)$', cond)
        assert cm and cm.group(1) == var, hdr
        op, end = cm.group(2), cm.group(3)
        st = step
        start_e, end_e = self.expr(start), self.expr(end)
        if st in (var + '++', '++' + var) and op in ('<', '<='):
            return 'for %s in %s..%s%s' % (var, start_e, '=' if op == '<=' else '', end_e)
        if st in (var + '--', '--' + var) and op in ('>=', '>'):
            lo = end_e if op == '>=' else '(%s) + 1' % end_e
            return 'for %s in (%s..=%s).rev()' % (var, lo, start_e)
        sm = re.match(r'^%s\s*\+=\s*(.*)$' % var, st)
        if sm and op in ('<', '<='):
            return 'for %s in (%s..%s%s).step_by((%s) as usize)' % (var, start_e, '=' if op == '<=' else '', end_e, self.expr(sm.group(1)))
        raise ValueError('for: ' + hdr)

def finish(rust_lines):
    """merge ELSE / ELSEIF markers with preceding closing brace; add opening braces."""
    out = []
    lines = rust_lines.split('\n')
    i = 0
    res = []
    for ln in lines:
        res.append(ln)
    # place '{' after control headers: control header lines are followed by '{' line already
    text = '\n'.join(res)
    text = re.sub(r'\}\nELSEIF (.*)\n\{', r'} else if \1 {', text)
    text = re.sub(r'\}\nELSE\n\{', r'} else {', text)
    text = re.sub(r'^(if|while|for) (.*)\n\{', r'\1 \2 {', text, flags=re.M)
    text = re.sub(r'^(if|while|for) (.*)\n\{', r'\1 \2 {', text, flags=re.M)
    text = re.sub(r'\}\n\{', '}\n{', text)
    return text
