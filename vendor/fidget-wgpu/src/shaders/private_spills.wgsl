struct SpillMemory {
    values: array<Value, MEM_COUNT>,
}

fn spill_load(memory: ptr<function, SpillMemory>, index: u32) -> Value {
    return memory.values[index];
}

fn spill_store(memory: ptr<function, SpillMemory>, index: u32, value: Value) {
    memory.values[index] = value;
}
