use deno_core::{extension, op2, JsRuntime, RuntimeOptions};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileResult {
    pub javascript: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub message: String,
    pub line: Option<usize>,
    pub start: Option<usize>,
    pub length: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickInfo {
    pub display_string: String,
    pub documentation: String,
    pub start: usize,
    pub length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeInfo {
    pub kind: String,              // "string", "number", "function", "union", etc.
    pub display_string: String,    // Human-readable type
    pub is_primitive: bool,
    pub is_union: bool,
    pub is_intersection: bool,
    pub union_types: Option<Vec<String>>,      // If it's a union
    pub properties: Option<Vec<PropertyInfo>>,  // Object properties
    pub call_signatures: Option<Vec<String>>,   // Function signatures
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyInfo {
    pub name: String,
    pub type_string: String,
    pub optional: bool,
}

// Shared state for passing values from JS back to Rust
thread_local! {
    static RETURN_VALUE: RefCell<Option<String>> = RefCell::new(None);
}

// Define the extension with our ops
extension!(
    ts_compiler,
    ops = [op_log_from_js, op_return_value],
);

/// Op that JS can call to log messages back to Rust
#[op2]
#[string]
fn op_log_from_js(#[string] message: String) -> String {
    println!("[JS]: {}", message);
    format!("Logged: {}", message)
}

/// Op that JS calls to "return" a value to Rust
#[op2(fast)]
fn op_return_value(#[string] value: String) {
    RETURN_VALUE.with(|cell| {
        *cell.borrow_mut() = Some(value);
    });
}

pub struct TypeScriptRuntime {
    runtime: JsRuntime,
}

impl TypeScriptRuntime {
    pub fn new() -> Result<Self, String> {
        let ext = ts_compiler::init();

        let mut runtime = JsRuntime::new(RuntimeOptions {
            extensions: vec![ext],
            ..Default::default()
        });

        // Set up console.log
        runtime
            .execute_script(
                "setup_console",
                r#"
                globalThis.console = {
                    log: (...args) => Deno.core.ops.op_log_from_js(args.join(' '))
                };
                "#,
            )
            .map_err(|e| format!("Failed to setup console: {:?}", e))?;

        Ok(Self { runtime })
    }

    /// Call a JS function and get the result back via op_return_value
    pub fn call_js_function(&mut self, fn_name: &str, arg: &str) -> Result<String, String> {
        // Clear any previous return value
        RETURN_VALUE.with(|cell| *cell.borrow_mut() = None);

        let arg_json = serde_json::to_string(arg).unwrap();

        let script = format!(
            r#"
            (function() {{
                const result = globalThis.{}({});
                Deno.core.ops.op_return_value(JSON.stringify(result));
            }})()
            "#,
            fn_name, arg_json
        );

        self.runtime
            .execute_script("<call>", script)
            .map_err(|e| format!("JS call error: {:?}", e))?;

        // Retrieve the returned value
        RETURN_VALUE.with(|cell| {
            cell.borrow()
                .clone()
                .ok_or_else(|| "No value returned from JS".to_string())
        })
    }

    /// Execute JavaScript code
    pub fn execute(&mut self, js_code: &str) -> Result<(), String> {
        self.runtime
            .execute_script("<anon>", js_code.to_string())
            .map_err(|e| format!("Execution error: {:?}", e))?;

        Ok(())
    }

    /// Load the TypeScript compiler into the runtime
    pub fn load_typescript_compiler(&mut self) -> Result<(), String> {
        println!("Loading TypeScript compiler...");

        // Load the real TypeScript compiler (8.6MB)
        self.execute(include_str!("typescript.js"))?;

        // Set up the compile function that uses TSC with full type checking
        self.execute(
            r#"
            globalThis.compileTypeScript = function(source) {
                try {
                    // Create an in-memory source file
                    const fileName = "input.ts";
                    const sourceFile = ts.createSourceFile(
                        fileName,
                        source,
                        ts.ScriptTarget.ES2020,
                        true
                    );

                    // Create a compiler host
                    const compilerHost = {
                        getSourceFile: (name) => name === fileName ? sourceFile : undefined,
                        writeFile: () => {},
                        getCurrentDirectory: () => "",
                        getDirectories: () => [],
                        fileExists: (name) => name === fileName,
                        readFile: (name) => name === fileName ? source : undefined,
                        getCanonicalFileName: (name) => name,
                        useCaseSensitiveFileNames: () => true,
                        getNewLine: () => "\n",
                        getDefaultLibFileName: () => "lib.d.ts"
                    };

                    // Create program for type checking
                    const program = ts.createProgram(
                        [fileName],
                        {
                            target: ts.ScriptTarget.ES2020,
                            module: ts.ModuleKind.ESNext,
                            noEmit: false
                        },
                        compilerHost
                    );

                    // Get diagnostics (type errors)
                    const diagnostics = [
                        ...program.getSemanticDiagnostics(sourceFile),
                        ...program.getSyntacticDiagnostics(sourceFile)
                    ];

                    // Transpile to JavaScript
                    const transpileResult = ts.transpileModule(source, {
                        compilerOptions: {
                            target: ts.ScriptTarget.ES2020,
                            module: ts.ModuleKind.ESNext,
                        }
                    });

                    return {
                        javascript: transpileResult.outputText,
                        diagnostics: diagnostics.map(d => ({
                            message: typeof d.messageText === 'string'
                                ? d.messageText
                                : d.messageText.messageText,
                            line: d.start !== undefined && d.file
                                ? d.file.getLineAndCharacterOfPosition(d.start).line
                                : null,
                            start: d.start,
                            length: d.length
                        }))
                    };
                } catch (e) {
                    return {
                        javascript: "",
                        diagnostics: [{
                            message: e.toString(),
                            line: null
                        }]
                    };
                }
            };
            "#,
        )?;

        // Set up the language service for IDE features (hover, completion, etc.)
        self.execute(
            r#"
            globalThis.createLanguageService = function(source) {
                const fileName = "input.ts";

                // Create a language service host
                const servicesHost = {
                    getScriptFileNames: () => [fileName],
                    getScriptVersion: () => "1",
                    getScriptSnapshot: (name) => {
                        if (name === fileName) {
                            return ts.ScriptSnapshot.fromString(source);
                        }
                        return undefined;
                    },
                    getCurrentDirectory: () => "",
                    getCompilationSettings: () => ({
                        target: ts.ScriptTarget.ES2020,
                        module: ts.ModuleKind.ESNext,
                        noLib: true,  // Don't require lib.d.ts
                    }),
                    getDefaultLibFileName: () => "",  // No lib file
                    fileExists: (name) => name === fileName,
                    readFile: (name) => name === fileName ? source : undefined,
                    directoryExists: () => true,
                    getDirectories: () => [],
                };

                const languageService = ts.createLanguageService(servicesHost, ts.createDocumentRegistry());

                return {
                    service: languageService,
                    fileName: fileName
                };
            };

            globalThis.getQuickInfoAtPosition = function(source, position) {
                const ls = createLanguageService(source);
                const quickInfo = ls.service.getQuickInfoAtPosition(ls.fileName, position);

                if (!quickInfo) {
                    return null;
                }

                return {
                    display_string: ts.displayPartsToString(quickInfo.displayParts || []),
                    documentation: ts.displayPartsToString(quickInfo.documentation || []),
                    start: quickInfo.textSpan.start,
                    length: quickInfo.textSpan.length
                };
            };

            globalThis.getStructuredTypeInfo = function(source, position) {
                const ls = createLanguageService(source);
                const program = ls.service.getProgram();
                if (!program) return null;

                const sourceFile = program.getSourceFile(ls.fileName);
                if (!sourceFile) return null;

                const checker = program.getTypeChecker();

                // Find the node at position
                function findNodeAtPosition(node, pos) {
                    if (pos >= node.pos && pos < node.end) {
                        let child = ts.forEachChild(node, n => findNodeAtPosition(n, pos));
                        return child || node;
                    }
                }

                const node = findNodeAtPosition(sourceFile, position);
                if (!node) return null;

                const type = checker.getTypeAtLocation(node);
                if (!type) return null;

                // Extract structured type information
                const typeFlags = type.flags;
                const result = {
                    kind: checker.typeToString(type),
                    display_string: checker.typeToString(type),
                    is_primitive: !!(typeFlags & (
                        ts.TypeFlags.String |
                        ts.TypeFlags.Number |
                        ts.TypeFlags.Boolean |
                        ts.TypeFlags.Null |
                        ts.TypeFlags.Undefined
                    )),
                    is_union: !!(typeFlags & ts.TypeFlags.Union),
                    is_intersection: !!(typeFlags & ts.TypeFlags.Intersection),
                    union_types: null,
                    properties: null,
                    call_signatures: null
                };

                // If it's a union type, extract member types
                if (result.is_union && type.types) {
                    result.union_types = type.types.map(t => checker.typeToString(t));
                }

                // If it's an object, extract properties
                const properties = type.getProperties();
                if (properties && properties.length > 0) {
                    result.properties = properties.map(prop => {
                        const propType = checker.getTypeOfSymbolAtLocation(prop, node);
                        return {
                            name: prop.getName(),
                            type_string: checker.typeToString(propType),
                            optional: !!(prop.flags & ts.SymbolFlags.Optional)
                        };
                    });
                }

                // If it's a function, extract call signatures
                const callSignatures = type.getCallSignatures();
                if (callSignatures && callSignatures.length > 0) {
                    result.call_signatures = callSignatures.map(sig =>
                        checker.signatureToString(sig)
                    );
                }

                return result;
            };
            "#,
        )?;

        println!("TypeScript compiler loaded successfully");
        Ok(())
    }

    /// Compile TypeScript code to JavaScript
    pub fn compile_typescript(&mut self, ts_code: &str) -> Result<CompileResult, String> {
        let result_json = self.call_js_function("compileTypeScript", ts_code)?;
        serde_json::from_str(&result_json).map_err(|e| format!("Failed to parse result: {}", e))
    }

    /// Get type information at a specific position (for hover tooltips)
    pub fn get_quick_info(&mut self, ts_code: &str, position: usize) -> Result<Option<QuickInfo>, String> {
        // Store the source code temporarily
        let script = format!(
            r#"
            (function() {{
                const source = {};
                const position = {};
                const result = getQuickInfoAtPosition(source, position);
                Deno.core.ops.op_return_value(JSON.stringify(result));
            }})()
            "#,
            serde_json::to_string(ts_code).unwrap(),
            position
        );

        RETURN_VALUE.with(|cell| *cell.borrow_mut() = None);
        self.runtime.execute_script("<quickinfo>", script)
            .map_err(|e| format!("Quick info error: {:?}", e))?;

        let result_json = RETURN_VALUE.with(|cell| {
            cell.borrow().clone().ok_or_else(|| "No result".to_string())
        })?;

        if result_json == "null" {
            return Ok(None);
        }

        Ok(serde_json::from_str(&result_json).map_err(|e| format!("Parse error: {}", e))?)
    }

    /// Get structured type information at a specific position
    pub fn get_structured_type_info(&mut self, ts_code: &str, position: usize) -> Result<Option<TypeInfo>, String> {
        let script = format!(
            r#"
            (function() {{
                const source = {};
                const position = {};
                const result = getStructuredTypeInfo(source, position);
                Deno.core.ops.op_return_value(JSON.stringify(result));
            }})()
            "#,
            serde_json::to_string(ts_code).unwrap(),
            position
        );

        RETURN_VALUE.with(|cell| *cell.borrow_mut() = None);
        self.runtime.execute_script("<typeinfo>", script)
            .map_err(|e| format!("Structured type info error: {:?}", e))?;

        let result_json = RETURN_VALUE.with(|cell| {
            cell.borrow().clone().ok_or_else(|| "No result".to_string())
        })?;

        if result_json == "null" {
            return Ok(None);
        }

        Ok(serde_json::from_str(&result_json).map_err(|e| format!("Parse error: {}", e))?)
    }

    /// Execute JavaScript and get the result by wrapping in a function
    pub fn execute_and_get_result(&mut self, js_code: &str) -> Result<String, String> {
        RETURN_VALUE.with(|cell| *cell.borrow_mut() = None);

        let wrapped = format!(
            r#"
            (function() {{
                const result = {};
                Deno.core.ops.op_return_value(JSON.stringify(result));
            }})()
            "#,
            js_code
        );

        self.execute(&wrapped)?;

        RETURN_VALUE.with(|cell| {
            cell.borrow()
                .clone()
                .ok_or_else(|| "No value returned".to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_creation() {
        let runtime = TypeScriptRuntime::new();
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_basic_js_execution() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        runtime.execute("1 + 1").unwrap();
    }

    #[test]
    fn test_console_log() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        runtime.execute("console.log('Hello from JS!')").unwrap();
    }

    #[test]
    fn test_return_value() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        let result = runtime.execute_and_get_result("42 + 58").unwrap();
        assert_eq!(result, "100");
    }

    #[test]
    fn test_return_object() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        let result = runtime
            .execute_and_get_result(r#"({value: 42, name: "test"})"#)
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["value"], 42);
        assert_eq!(parsed["name"], "test");
    }

    #[test]
    fn test_runtime_error() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        let result = runtime.execute("throw new Error('test error')");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_typescript_compiler() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        runtime.load_typescript_compiler().unwrap();
    }

    #[test]
    fn test_compile_simple_typescript() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        runtime.load_typescript_compiler().unwrap();

        let ts_code = "const x: number = 42; const y: number = 58; x + y";
        let result = runtime.compile_typescript(ts_code).unwrap();

        println!("Compiled JS:\n{}", result.javascript);
        println!("Diagnostics: {:?}", result.diagnostics);

        // Should have compiled successfully
        assert!(result.diagnostics.is_empty());
        assert!(result.javascript.contains("42"));
        assert!(result.javascript.contains("58"));
        // Type annotations should be stripped
        assert!(!result.javascript.contains(": number"));
    }

    #[test]
    fn test_compile_and_execute_typescript() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        runtime.load_typescript_compiler().unwrap();

        // Compile TypeScript function
        let ts_code = r#"
            function add(a: number, b: number): number {
                return a + b;
            }
        "#;

        let result = runtime.compile_typescript(ts_code).unwrap();
        assert!(result.diagnostics.is_empty(), "Should have no errors");

        // Load the compiled function into global scope
        runtime.execute(&result.javascript).unwrap();

        // Now call the function
        let exec_result = runtime.execute_and_get_result("add(10, 32)").unwrap();
        assert_eq!(exec_result, "42");
    }

    #[test]
    fn test_typescript_type_error() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        runtime.load_typescript_compiler().unwrap();

        // Code with a type error
        let ts_code = r#"const x: number = "string";"#;
        let result = runtime.compile_typescript(ts_code).unwrap();

        println!("Diagnostics for type error: {:?}", result.diagnostics);

        // Should detect the type error
        assert!(!result.diagnostics.is_empty());
        assert!(result.diagnostics[0]
            .message
            .to_lowercase()
            .contains("string"));

        // Should have position information for squiggles
        assert!(result.diagnostics[0].start.is_some());
        assert!(result.diagnostics[0].length.is_some());
    }

    #[test]
    fn test_get_type_info() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        runtime.load_typescript_compiler().unwrap();

        let ts_code = r#"const x: number = 42;"#;

        // Get type info for variable 'x' (position 6 is on the 'x')
        let info = runtime.get_quick_info(ts_code, 6).unwrap();

        println!("Type info at position 6: {:?}", info);

        assert!(info.is_some());
        let info = info.unwrap();

        // Should show it's a number
        assert!(info.display_string.contains("number") || info.display_string.contains("const"));
    }

    #[test]
    fn test_hover_on_function() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        runtime.load_typescript_compiler().unwrap();

        let ts_code = r#"
            function add(a: number, b: number): number {
                return a + b;
            }
            const result = add(1, 2);
        "#;

        // Get type info for the function call 'add' (find position of 'add' in the call)
        let add_call_pos = ts_code.find("add(1").unwrap();
        let info = runtime.get_quick_info(ts_code, add_call_pos).unwrap();

        println!("Hover info on function: {:?}", info);

        assert!(info.is_some());
        let info = info.unwrap();

        // Should show the function signature
        assert!(info.display_string.contains("add") || info.display_string.contains("number"));
    }

    #[test]
    fn test_structured_type_info_primitive() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        runtime.load_typescript_compiler().unwrap();

        let ts_code = r#"const x: number = 42;"#;

        // Get structured type info for 'x' at position 6
        let info = runtime.get_structured_type_info(ts_code, 6).unwrap();

        println!("Structured type info for number: {:?}", info);

        assert!(info.is_some());
        let info = info.unwrap();

        assert!(info.is_primitive);
        assert!(!info.is_union);
        assert!(!info.is_intersection);
        assert!(info.display_string.contains("number"));
    }

    #[test]
    fn test_structured_type_info_union() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        runtime.load_typescript_compiler().unwrap();

        let ts_code = r#"const x: string | number = 42;"#;

        // Get structured type info for 'x'
        let x_pos = ts_code.find("x:").unwrap();
        let info = runtime.get_structured_type_info(ts_code, x_pos).unwrap();

        println!("Structured type info for union: {:?}", info);

        assert!(info.is_some());
        let info = info.unwrap();

        assert!(info.is_union);
        assert!(info.union_types.is_some());
        let union_types = info.union_types.unwrap();
        assert_eq!(union_types.len(), 2);
        // Union types should be string and number (order may vary)
        assert!(union_types.contains(&"string".to_string()) || union_types.contains(&"number".to_string()));
    }

    #[test]
    fn test_structured_type_info_object() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        runtime.load_typescript_compiler().unwrap();

        let ts_code = r#"
            const person = {
                name: "Alice",
                age: 30,
                email: "alice@example.com"
            };
        "#;

        // Get structured type info for 'person'
        let person_pos = ts_code.find("person").unwrap();
        let info = runtime.get_structured_type_info(ts_code, person_pos).unwrap();

        println!("Structured type info for object: {:?}", info);

        assert!(info.is_some());
        let info = info.unwrap();

        assert!(!info.is_primitive);
        assert!(info.properties.is_some());
        let properties = info.properties.unwrap();

        // Should have name, age, and email properties
        assert_eq!(properties.len(), 3);

        let prop_names: Vec<&str> = properties.iter().map(|p| p.name.as_str()).collect();
        assert!(prop_names.contains(&"name"));
        assert!(prop_names.contains(&"age"));
        assert!(prop_names.contains(&"email"));

        // Check types
        let name_prop = properties.iter().find(|p| p.name == "name").unwrap();
        assert!(name_prop.type_string.contains("string"));

        let age_prop = properties.iter().find(|p| p.name == "age").unwrap();
        assert!(age_prop.type_string.contains("number"));
    }

    #[test]
    fn test_structured_type_info_function() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        runtime.load_typescript_compiler().unwrap();

        let ts_code = r#"
            function add(a: number, b: number): number {
                return a + b;
            }
        "#;

        // Get structured type info for 'add'
        let add_pos = ts_code.find("add").unwrap();
        let info = runtime.get_structured_type_info(ts_code, add_pos).unwrap();

        println!("Structured type info for function: {:?}", info);

        assert!(info.is_some());
        let info = info.unwrap();

        assert!(info.call_signatures.is_some());
        let signatures = info.call_signatures.unwrap();
        assert!(!signatures.is_empty());

        // Signature should contain parameter and return type info
        let sig = &signatures[0];
        assert!(sig.contains("number"));
    }

    #[test]
    fn test_structured_type_info_optional_property() {
        let mut runtime = TypeScriptRuntime::new().unwrap();
        runtime.load_typescript_compiler().unwrap();

        let ts_code = r#"
            interface User {
                name: string;
                email?: string;
            }
            const user: User = { name: "Bob" };
        "#;

        // Get structured type info for 'user'
        let user_pos = ts_code.find("user:").unwrap();
        let info = runtime.get_structured_type_info(ts_code, user_pos).unwrap();

        println!("Structured type info for interface with optional: {:?}", info);

        assert!(info.is_some());
        let info = info.unwrap();

        assert!(info.properties.is_some());
        let properties = info.properties.unwrap();

        let name_prop = properties.iter().find(|p| p.name == "name").unwrap();
        assert!(!name_prop.optional);

        let email_prop = properties.iter().find(|p| p.name == "email").unwrap();
        assert!(email_prop.optional);
    }
}
