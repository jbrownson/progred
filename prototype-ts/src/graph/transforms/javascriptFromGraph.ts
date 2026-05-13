import { bindMaybe, Maybe, nothing, sequenceMaybe } from "../../lib/Maybe"
import {
  And, ArrayLiteral, ArrowFunction, Assignment, BinaryInline, BinaryOperator, Conditional, Const, ConstLetVar, Difference, Dot, Equals, Extern, Expression, FunctionCall, FunctionDeclaration, GreaterThan, GreaterThanOrEqualTo, HasNID, HasSID, If, JavaScriptProgram, KeyValue, LessThan, LessThanOrEqualTo, Let, matchBinaryOperator, matchConstLetVar, New, NotEquals, Null, ObjectLiteral, Or, Parameter, Product, Quotient, Return, Statement, StrictEquals, StrictNotEquals, Sum, Undefined, Var, VariableDeclaration
} from "../graph"
import { GUID, guidFromID, ID } from "../model/ID"

type Scope = Map<string, ID>

function block(statements: Maybe<Statement[]>, scopes: Scope[]): Maybe<string> {
  return bindMaybe(statements, statements => {
    let scope = statementScope(statements)
    return bindMaybe(sequenceMaybe(statements.map((statement, i) => () => statementFromGraph(statement, [scope, ...scopes], i + 1 === statements.length))), statements =>
      `{\n${statements.map(statement => `  ${statement}`).join("\n")}\n}`) }) }

function declarationName(x: {name: Maybe<string>, id: ID}): [string, ID][] {
  return x.name ? [[x.name, x.id]] : [] }

function identifierFor(id: ID): Maybe<string> {
  return bindMaybe(guidFromID(id), guid => identifierForGUID(guid)) }

function identifierForGUID(guid: GUID) {
  return `_${guid.replace(/[^A-Za-z0-9_$]/g, "_")}` }

function statementScope(statements: Statement[]): Scope {
  return new Map(statements.flatMap(statement =>
    statement instanceof FunctionDeclaration ? declarationName(statement)
      : statement instanceof VariableDeclaration ? declarationName(statement)
      : [])) }

function parameterScope(parameters: Parameter[]): Scope {
  return new Map(parameters.flatMap(declarationName)) }

function lookupName(name: string, scopes: Scope[]): Maybe<ID> {
  for (let scope of scopes) {
    let id = scope.get(name)
    if (id !== undefined) return id }
  return nothing }

function expressionList(expressions: Maybe<Expression[]>, scopes: Scope[]): Maybe<string> {
  return bindMaybe(expressions, expressions => bindMaybe(sequenceMaybe(expressions.map(expression => () => expressionFromGraph(expression, scopes))), expressions =>
    expressions.join(", "))) }

function parametersFromGraph(parameters: Maybe<Parameter[]>): Maybe<string> {
  return bindMaybe(parameters, parameters => bindMaybe(sequenceMaybe(parameters.map(parameter => () => identifierFor(parameter.id))), parameters =>
    parameters.join(", "))) }

function statementsAsBlock(statements: Maybe<Statement[]>, scopes: Scope[]): Maybe<string> {
  return block(statements, scopes) }

function rawIdentifier(s: string): Maybe<string> {
  return /^[$A-Z_a-z][$\w]*(\.[_$A-Z_a-z][$\w]*)*$/.test(s) ? s : nothing }

function stringExpression(s: string, scopes: Scope[]) {
  return bindMaybe(lookupName(s, scopes), identifierFor) || JSON.stringify(s) }

function binaryOperatorFromGraph(binaryOperator: BinaryOperator) {
  return matchBinaryOperator(binaryOperator,
    (_: Sum) => "+",
    (_: Product) => "*",
    (_: Quotient) => "/",
    (_: Difference) => "-",
    (_: And) => "&&",
    (_: Or) => "||",
    (_: Dot) => ".",
    (_: Equals) => "==",
    (_: NotEquals) => "!=",
    (_: StrictEquals) => "===",
    (_: StrictNotEquals) => "!==",
    (_: GreaterThan) => ">",
    (_: LessThan) => "<",
    (_: GreaterThanOrEqualTo) => ">=",
    (_: LessThanOrEqualTo) => "<=",
    (_: Assignment) => "=") }

function propertyAccess(x: Expression, scopes: Scope[]): Maybe<string> {
  if (x instanceof HasSID) {
    let property = x.string
    return /^[$A-Z_a-z][$\w]*$/.test(property) ? `.${property}` : `[${JSON.stringify(property)}]` }
  return bindMaybe(expressionFromGraph(x, scopes), expression => `[${expression}]`) }

function binaryInlineFromGraph(binaryInline: BinaryInline, scopes: Scope[]): Maybe<string> {
  return bindMaybe(binaryInline.left, left => bindMaybe(binaryInline.binaryOperator, binaryOperator => bindMaybe(binaryInline.right, right => {
    let operator = binaryOperatorFromGraph(binaryOperator)
    return operator === "."
      ? bindMaybe(expressionFromGraph(left, scopes), left => bindMaybe(propertyAccess(right, scopes), right => `(${left}${right})`))
      : bindMaybe(expressionFromGraph(left, scopes), left => bindMaybe(expressionFromGraph(right, scopes), right => `(${left} ${operator} ${right})`)) }))) }

function functionDeclarationStatement(functionDeclaration: FunctionDeclaration, scopes: Scope[]): Maybe<string> {
  return bindMaybe(identifierFor(functionDeclaration.id), identifier => bindMaybe(functionDeclaration.parameters, parameters =>
    bindMaybe(parametersFromGraph(parameters), parametersText =>
      bindMaybe(statementsAsBlock(functionDeclaration.statements, [parameterScope(parameters), ...scopes]), body =>
        `function ${identifier}(${parametersText}) ${body}`)))) }

function constLetVarFromGraph(constLetVar: ConstLetVar) {
  return matchConstLetVar(constLetVar,
    (_: Const) => "const",
    (_: Let) => "let",
    (_: Var) => "var") }

function expressionStatement(expression: Expression, scopes: Scope[], isLast: boolean): Maybe<string> {
  return bindMaybe(expressionFromGraph(expression, scopes), expression => isLast ? expression : `${expression};`) }

export function statementFromGraph(statement: Statement, scopes: Scope[] = [], isLast = true): Maybe<string> {
  return statement instanceof FunctionDeclaration ? functionDeclarationStatement(statement, scopes)
    : statement instanceof Return ? bindMaybe(statement.expression, expression => bindMaybe(expressionFromGraph(expression, scopes), expression => `return ${expression}`))
    : statement instanceof If ? bindMaybe(statement.condition, condition => bindMaybe(expressionFromGraph(condition, scopes), condition =>
      bindMaybe(statementsAsBlock(statement.trueStatements, scopes), trueStatements => bindMaybe(statementsAsBlock(statement.falseStatements, scopes), falseStatements =>
        `if (${condition}) ${trueStatements} else ${falseStatements}`))))
    : statement instanceof VariableDeclaration ? bindMaybe(statement.constLetVar, constLetVar => bindMaybe(identifierFor(statement.id), identifier =>
      bindMaybe(statement.expression, expression => bindMaybe(expressionFromGraph(expression, scopes), expression =>
        `${constLetVarFromGraph(constLetVar)} ${identifier} = ${expression};`))))
    : expressionStatement(statement, scopes, isLast) }

export function expressionFromGraph(expression: Expression, scopes: Scope[] = []): Maybe<string> {
  return expression instanceof FunctionDeclaration ? identifierFor(expression.id)
    : expression instanceof Extern ? bindMaybe(expression.name, rawIdentifier)
    : expression instanceof Parameter ? identifierFor(expression.id)
    : expression instanceof ArrayLiteral ? bindMaybe(expressionList(expression.expressions, scopes), expressions => `[${expressions}]`)
    : expression instanceof ObjectLiteral ? bindMaybe(expression.keyValues, keyValues => bindMaybe(sequenceMaybe(keyValues.map(keyValue => () => keyValueFromGraph(keyValue, scopes))), keyValues => `({${keyValues.join(", ")}})`))
    : expression instanceof KeyValue ? bindMaybe(keyValueFromGraph(expression, scopes), keyValue => `({${keyValue}})`)
    : expression instanceof BinaryInline ? binaryInlineFromGraph(expression, scopes)
    : expression instanceof Conditional ? bindMaybe(expression.condition, condition => bindMaybe(expression.trueExpression, trueExpression => bindMaybe(expression.falseExpression, falseExpression =>
      bindMaybe(expressionFromGraph(condition, scopes), condition => bindMaybe(expressionFromGraph(trueExpression, scopes), trueExpression => bindMaybe(expressionFromGraph(falseExpression, scopes), falseExpression =>
        `(${condition} ? ${trueExpression} : ${falseExpression})`))))))
    : expression instanceof FunctionCall ? bindMaybe(expression.function, f => bindMaybe(expressionFromGraph(f, scopes), f => bindMaybe(expressionList(expression.arguments, scopes), args => `${f}(${args})`)))
    : expression instanceof ArrowFunction ? bindMaybe(expression.parameters, parameters => bindMaybe(parametersFromGraph(parameters), parametersText =>
      bindMaybe(statementsAsBlock(expression.statements, [parameterScope(parameters), ...scopes]), body => `((${parametersText}) => ${body})`)))
    : expression instanceof New ? bindMaybe(expression.expression, newExpression => bindMaybe(expressionFromGraph(newExpression, scopes), newExpression => bindMaybe(expressionList(expression.arguments, scopes), args => `new ${newExpression}(${args})`)))
    : expression instanceof Undefined ? "undefined"
    : expression instanceof Null ? "null"
    : expression instanceof HasNID ? `${expression.number}`
    : stringExpression(expression.string, scopes) }

function keyValueFromGraph(keyValue: KeyValue, scopes: Scope[]): Maybe<string> {
  return bindMaybe(keyValue.objectKey, key => bindMaybe(keyValue.objectValue, value => bindMaybe(expressionFromGraph(value, scopes), value =>
    `${JSON.stringify(key)}: ${value}`))) }

export function javascriptFromGraph(javascriptProgram: JavaScriptProgram): Maybe<string> {
  return bindMaybe(javascriptProgram.statements, statements => block(statements, [statementScope(statements)])) }
