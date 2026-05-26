import { runGraphCLI } from "./graphCLI"

const result = runGraphCLI(process.argv.slice(2))
if (result.stdout) console.log(result.stdout)
if (result.stderr) console.error(result.stderr)
process.exit(result.exitCode)
