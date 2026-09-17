// Each invocation owns its lane for the entire pass. Strided work reuses it.
struct SpillMemory { unused: u32 }

fn spill_load(memory: ptr<function, SpillMemory>, index: u32) -> Value {
    let base = config.spill_base + index * config.spill_lanes * VALUE_COMPONENTS + spill_lane;
    let stride = config.spill_lanes;
    var value = vec4f(var_values[base], 0.0, 0.0, 0.0);
    if VALUE_COMPONENTS >= 2u { value.y = var_values[base + stride]; }
    if VALUE_COMPONENTS == 4u {
        value.z = var_values[base + 2u * stride];
        value.w = var_values[base + 3u * stride];
    }
    return unpack_spill(value);
}

fn spill_store(memory: ptr<function, SpillMemory>, index: u32, value: Value) {
    let base = config.spill_base + index * config.spill_lanes * VALUE_COMPONENTS + spill_lane;
    let stride = config.spill_lanes;
    let v = pack_spill(value);
    var_values[base] = v.x;
    if VALUE_COMPONENTS >= 2u { var_values[base + stride] = v.y; }
    if VALUE_COMPONENTS == 4u {
        var_values[base + 2u * stride] = v.z;
        var_values[base + 3u * stride] = v.w;
    }
}
