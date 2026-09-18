# A deliberately limited interval interpreter for the real CAM fixture tapes.
# The immutable tape is shared. Every leaf owns and reuses one mutable array.
import Base

type Ival is Data:
  Iv{lo: F32, hi: F32}

type Tape is Data:
  End{}
  Step{code: Nat, dst: U32, a: U32, b: U32, next: Tape}

type Results is Data:
  RowsEnd{}
  Row{lo: U32, hi: U32, hash: U32, next: Results}
  Branch{left: Results, right: Results}

type Config is Data:
  Config{tape: Tape, slots: Nat, base: U32, count: Nat, depth: Nat}

def load() -> IO(Config):
  import "./io.c"

def start() -> IO(Unit):
  import "./io.c"

def report(rows: Results) -> IO(Unit):
  import "./io.c"

def from_bits(x: U32) -> F32:
  match x:
    case U32{w}:
      F32{w}

def nan() -> Ival:
  Iv{from_bits(2143289344), from_bits(2143289344)}

def invalid(+l: F32, +h: F32) -> Bool:
  Bool.or(F32.is_ne(l, l), F32.is_ne(h, h))

def neg(v: Ival) -> Ival:
  match v:
    case Iv{l, h}:
      Iv{F32.neg(h), F32.neg(l)}

def square(v: Ival) -> Ival:
  match v:
    case Iv{+l, +h}:
      Bool.pick(Ival, F32.is_lt(h, 0.0), Iv{F32.square(h), F32.square(l)},
        Bool.pick(Ival, F32.is_gt(l, 0.0), Iv{F32.square(l), F32.square(h)},
          Bool.pick(Ival, invalid(l, h), nan(),
            Iv{0.0, F32.square(F32.max(F32.abs(l), F32.abs(h)))})))

def add(a: Ival, b: Ival) -> Ival:
  match a b:
    case Iv{l, h} Iv{x, y}:
      Iv{F32.add(l, x), F32.add(h, y)}

def sub(a: Ival, b: Ival) -> Ival:
  match a b:
    case Iv{l, h} Iv{x, y}:
      Iv{F32.sub(l, y), F32.sub(h, x)}

def mul_const(a: Ival, +b: F32) -> Ival:
  match a:
    case Iv{+l, +h}:
      Bool.pick(Ival, Bool.or(invalid(l, h), F32.is_ne(b, b)), nan(),
        Bool.pick(Ival, F32.is_lt(b, 0.0), Iv{F32.mul(h, b), F32.mul(l, b)},
          Iv{F32.mul(l, b), F32.mul(h, b)}))

def div_const(a: Ival, +b: F32) -> Ival:
  match a:
    case Iv{+l, +h}:
      +x = F32.div(l, b)
      +y = F32.div(h, b)
      Bool.pick(Ival, Bool.or(invalid(l, h), Bool.not(Bool.or(F32.is_gt(b, 0.0), F32.is_lt(b, 0.0)))), nan(),
        Iv{F32.min(x, y), F32.max(x, y)})

def choose(a: Bool, b: Bool) -> U32:
  Bool.pick(U32, a, 1, Bool.pick(U32, b, 2, 3))

def minmax(+max: Bool, a: Ival, b: Ival) -> Ival & U32:
  match a b:
    case Iv{+l, +h} Iv{+x, +y}:
      +bad = Bool.or(invalid(l, h), invalid(x, y))
      v = Bool.pick(Ival, max, Iv{F32.max(l, x), F32.max(h, y)}, Iv{F32.min(l, x), F32.min(h, y)})
      c = Bool.pick(U32, max, choose(F32.is_gt(l, y), F32.is_gt(x, h)), choose(F32.is_lt(h, x), F32.is_lt(y, l)))
      (Bool.pick(Ival, bad, nan(), v), Bool.pick(U32, bad, 3, c))

def put(ws: Array<Ival>, d: U32, v: Ival, hash: U32) -> Array<Ival> & U32:
  (Array.set(Ival, ws, d, v), hash)

def put_choice(ws: Array<Ival>, d: U32, r: Ival & U32, hash: U32) -> Array<Ival> & U32:
  (v, c) = r
  put(ws, d, v, U32.add(U32.mul(hash, 33), c))

# A computed pair is unpacked at an ordinary function boundary. Each
# continuation is called once; the reusable program itself is Tape data.
def unpack(-A: Type, -B: Type, -R: Type, pair: A & B, k: A -> B -> R) -> R:
  (a, b) = pair
  k(a, b)

# Read helpers destructure at function boundaries, so the hot instruction loop
# creates no runtime closures.
def read_two(code: Nat, d: U32, x: Ival, hash: U32, pair: Array<Ival> & Ival) -> Array<Ival> & U32:
  match code pair:
    case 0n Tuple{ws, y}:
      put(ws, d, add(x, y), hash)
    case 1n Tuple{ws, y}:
      put(ws, d, sub(x, y), hash)
    case 2n Tuple{ws, y}:
      put_choice(ws, d, minmax(False{}, x, y), hash)
    case 3n+p Tuple{ws, y}:
      put_choice(ws, d, minmax(True{}, x, y), hash)

# Encoding is specified in prepare.py. Unsupported Fidget opcodes fail export.
def read_one(code: Nat, d: U32, +b: U32, hash: U32, pair: Array<Ival> & Ival) -> Array<Ival> & U32:
  match code pair:
    case 0n Tuple{ws, x}:
      put(ws, d, x, hash)
    case 1n Tuple{ws, x}:
      put(ws, d, Iv{from_bits(b), from_bits(b)}, hash)
    case 2n Tuple{ws, x}:
      put(ws, d, neg(x), hash)
    case 3n Tuple{ws, x}:
      put(ws, d, square(x), hash)
    case 4n Tuple{ws, x}:
      read_two(0n, d, x, hash, Array.get(Ival, ws, b))
    case 5n Tuple{ws, x}:
      read_two(1n, d, x, hash, Array.get(Ival, ws, b))
    case 6n Tuple{ws, x}:
      read_two(2n, d, x, hash, Array.get(Ival, ws, b))
    case 7n Tuple{ws, x}:
      read_two(3n, d, x, hash, Array.get(Ival, ws, b))
    case 8n Tuple{ws, x}:
      put(ws, d, mul_const(x, from_bits(b)), hash)
    case 9n Tuple{ws, x}:
      put(ws, d, div_const(x, from_bits(b)), hash)
    case 10n Tuple{ws, x}:
      put(ws, d, sub(x, Iv{from_bits(b), from_bits(b)}), hash)
    case 11n Tuple{ws, x}:
      put(ws, d, sub(Iv{from_bits(b), from_bits(b)}, x), hash)
    case 12n Tuple{ws, x}:
      put_choice(ws, d, minmax(False{}, x, Iv{from_bits(b), from_bits(b)}), hash)
    case 13n+p Tuple{ws, x}:
      put_choice(ws, d, minmax(True{}, x, Iv{from_bits(b), from_bits(b)}), hash)

def eval(tape: Tape, state: Array<Ival> & U32) -> Array<Ival> & U32:
  match tape state:
    case End{} result:
      result
    case Step{code, d, a, b, tail} Tuple{ws, hash}:
      eval(tail, read_one(code, d, b, hash, Array.get(Ival, ws, a)))

def coord(i: U32, shift: Nat, +r: F32) -> Ival:
  +p = F32.div(F32.sub(U32.to_f32(U32.and(U32.shrn(i, shift), 15)), 7.5), 12.0)
  Iv{F32.sub(p, r), F32.add(p, r)}

def sample(+i: U32, +base: U32, ws: Array<Ival>) -> Array<Ival>:
  +r = Bool.pick(F32, U32.is_zero(U32.mod(i, 7)), 0.0, 0.0078125)
  +key = U32.and(U32.mul(i, 2654435761), 4095)
  ws = Array.set(Ival, ws, base, coord(key, 0n, r))
  ws = Array.set(Ival, ws, U32.add(base, 1), coord(key, 4n, r))
  Array.set(Ival, ws, U32.add(base, 2), coord(key, 8n, r))

def row(v: Ival, hash: U32, next: Results) -> Results:
  match v:
    case Iv{l, h}:
      Row{F32.bits(l), F32.bits(h), hash, next}

def loop(n: Nat, +i: U32, +tape: Tape, +base: U32, ws: Array<Ival>) -> Results:
  match n:
    case 0n:
      RowsEnd{}
    case 1n+p:
      ws = sample(i, base, ws)
      unpack(Array<Ival>, U32, Results, eval(tape, (ws, 0)), ws => h => unpack(Array<Ival>, Ival, Results, Array.get(Ival, ws, U32.add(base, 3)), ws => v => row(v, h, loop(p, U32.inc(i), tape, base, ws))))

def batch(depth: Nat, +i: U32, +n: Nat, +tape: Tape, +slots: Nat, +base: U32) -> Results:
  match depth:
    case 0n:
      loop(n, i, tape, base, Array.new(Ival, slots, Iv{0.0, 0.0}))
    case 1n+ +p:
      +half = Nat.div(n, 2n)
      l r = batch(p, i, half, tape, slots, base) batch(p, U32.add(i, U32.from_nat(half)), half, tape, slots, base)
      Branch{l, r}

def rounds(n: Nat, +tape: Tape, +slots: Nat, +base: U32, +count: Nat, +depth: Nat) -> IO(Unit):
  match n:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+p:
      do IO<Unit>:
        start()
        rows : Results = batch(depth, 0, count, tape, slots, base)
        report(rows)
        rounds(p, tape, slots, base, count, depth)

def run(c: Config) -> IO(Unit):
  match c:
    case Config{tape, slots, base, count, depth}:
      rounds(4n, tape, slots, base, count, depth)

def main() -> IO(Unit):
  do IO<Unit>:
    config : Config <- load()
    run(config)
