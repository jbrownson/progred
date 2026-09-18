"""Translate the test-only Fidget JSON into a runtime-loaded binary fixture.

All integers are little-endian u32 (this experiment targets Apple Silicon).
Header: magic, op count, log2 workspace slots, input base, samples, fork depth,
choice count. Then samples*(lower bits, upper bits, trace hash), then ops of
(opcode, destination, operand a, operand b / immediate f32 bits).
No expression reordering, constant folding, or DAG expansion occurs here.
"""
import argparse
import json
import math
from pathlib import Path
import struct
import time

OPCODES = {
    "CopyReg": 0, "CopyImm": 1, "NegReg": 2, "SquareReg": 3,
    "AddRegReg": 4, "SubRegReg": 5, "MinRegReg": 6, "MaxRegReg": 7,
    "MulRegImm": 8, "DivRegImm": 9, "SubRegImm": 10, "SubImmReg": 11,
    "MinRegImm": 12, "MaxRegImm": 13,
}

def prepare(source, output, depth):
    start = time.perf_counter()
    data = json.loads(source.read_text())
    base = data["slots"]
    slots = math.ceil(math.log2(base + 4))
    count = data["samples"]
    assert 0 <= depth <= 12 and count % (1 << depth) == 0
    ops = []
    for raw in data["ops"]:
        (name, args), = raw.items()
        if name == "Input":
            d, i = args
            op = [0, d, base + data["axes"][i], 0]
        elif name == "Output":
            a, i = args
            assert i == 0
            op = [0, base + 3, a, 0]
        elif name == "Load":
            d, a = args
            op = [0, d, a, 0]
        elif name == "Store":
            a, d = args
            op = [0, d, a, 0]
        else:
            code = OPCODES[name]  # fail closed on an unimplemented operation
            d = args[0]
            a = args[1]
            b = args[2] if len(args) == 3 else 0
            if "Imm" in name:
                b = struct.unpack("<I", struct.pack("<f", args[-1]))[0]
                if name == "CopyImm":
                    a = 0
            op = [code, d, a, b]
        assert op[1] < 1 << slots and (op[0] == 1 or op[2] < 1 << slots)
        if op[0] in [4, 5, 6, 7]:
            assert op[3] < 1 << slots
        ops.extend(op)
    words = [0x42464931, len(data["ops"]), slots, base, count, depth, data["choices"]]
    words += [v for row in data["expected"] for v in row]
    words += ops
    output.write_bytes(struct.pack(f"<{len(words)}I", *words))
    print(json.dumps({"file": str(output), "conversion_ms": (time.perf_counter()-start)*1000, "bytes": output.stat().st_size}))

if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("source", type=Path)
    p.add_argument("output", type=Path)
    p.add_argument("--depth", type=int, default=3)
    a = p.parse_args()
    prepare(a.source, a.output, a.depth)
