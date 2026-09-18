// Benchmark-only foreign effects for the pinned Bend runtime. No evaluation
// occurs here: C loads data, measures wall time, and checks Bend's results.
#include <sys/resource.h>

static u32 *expected;
static u32 sample_count, result_count, errors, round_number;
static u64 started;

static Term node_words(Env e, u32 cid, u32 n, const Term *words) {
  Loc l = heap_alloc(e, cls_fit(n));
  for (u32 i = 0; i < n; ++i) e.mem[l + i] = words[i];
  return term_ctr(cid, l);
}

static void read_words(u32 *p, size_t n) {
  if (fread(p, sizeof(u32), n, stdin) != n) err_fail("short benchmark input");
}

Term load_run(Env e, Term *f, IoWork *w) {
  u64 before = io_tick();
  u32 h[7];
  read_words(h, 7);
  if (h[0] != 0x42464931 || h[1] > 1000000 || h[2] > 24 || h[4] > 65536
      || h[5] > 12 || !h[4] || h[4] % (1u << h[5])) err_fail("bad benchmark header");
  sample_count = h[4];
  expected = io_mem(malloc(sample_count * 3 * sizeof(u32)));
  read_words(expected, sample_count * 3);
  u32 *ops = io_mem(malloc(h[1] * 4 * sizeof(u32)));
  read_words(ops, h[1] * 4);
  Term tape = term_pak(CID_END, 0);
  for (u32 i = h[1]; i > 0; --i) {
    u32 *op = ops + (i - 1) * 4;
    if (op[0] > 13) err_fail("bad benchmark opcode");
    // Shared constructors must carry sealed child pointers in this runtime.
    Term fields[] = {op[0], op[1], op[2], op[3], rfc_seal(e, tape)};
    tape = node_words(e, CID_STEP, 5, fields);
  }
  free(ops);
  Term fields[] = {rfc_seal(e, tape), h[2], h[3], h[4], h[5]};
  Term config = node_words(e, CID_CONFIG, 5, fields);
  printf("{\"load_ms\":%.6f,\"ops\":%u,\"samples\":%u,\"batch_depth\":%u}\n",
         (io_tick() - before) / 1e6, h[1], h[4], h[5]);
  return config;
}

Term start_run(Env e, Term *f, IoWork *w) {
  started = io_tick();
  return term_pak(CID_UNIT, 0);
}

static int same_float(u32 a, u32 b) {
  // NaN payload/sign are unspecified; all other bits, including zero, matter.
  return a == b || ((a & 0x7fffffff) > 0x7f800000 && (b & 0x7fffffff) > 0x7f800000);
}

static void check_rows(Env e, Term t) {
  while (term_aux(t) != CID_ROWSEND) {
    if (term_aux(t) == CID_BRANCH) {
      Term fs[2];
      spare_free(e, cls_fit(2), ctr_take(e, t, 2, fs));
      check_rows(e, fs[0]);
      t = fs[1];
    } else if (term_aux(t) == CID_ROW) {
      Term fs[4];
      spare_free(e, cls_fit(4), ctr_take(e, t, 4, fs));
      if (result_count >= sample_count) err_fail("too many results");
      u32 *want = expected + result_count * 3;
      if (!same_float(fs[0], want[0]) || !same_float(fs[1], want[1]) || fs[2] != want[2]) {
        if (errors++ < 3) fprintf(stderr, "sample %u: got %08x %08x %08x, want %08x %08x %08x\n",
            result_count, (u32)fs[0], (u32)fs[1], (u32)fs[2], want[0], want[1], want[2]);
      }
      result_count++;
      t = fs[3];
    } else err_fail("bad result constructor");
  }
}

Term report_run(Env e, Term *f, IoWork *w) {
  double ms = (io_tick() - started) / 1e6;
  result_count = errors = 0;
  check_rows(e, f[0]);
  if (result_count != sample_count) err_fail("missing results");
  struct rusage usage;
  getrusage(RUSAGE_SELF, &usage);
  printf("{\"round\":%u,\"ms\":%.6f,\"errors\":%u,\"max_rss_bytes\":%ld}\n",
         round_number++, ms, errors, usage.ru_maxrss);
  fflush(stdout);
  if (errors) err_fail("Bend results differ from Fidget");
  return term_pak(CID_UNIT, 0);
}

static void __attribute__((constructor)) benchmark_effects(void) {
  io_eff(CID_LOAD, load_run, 0);
  io_eff(CID_START, start_run, 0);
  io_eff(CID_REPORT, report_run, 0);
}
