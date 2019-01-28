import { altMaybe, bindMaybe, fromMaybe, mapMaybe, Maybe, nothing } from "../lib/Maybe"
import { arrayFromList } from "./arrayFromList"
import { _get, set, setOrDelete } from "./Environment"
import { generateGUID, GUID, guidFromID, ID, NID, nidFromID, nidFromNumber, numberFromID, numberFromNID, SID, sidFromID, sidFromString, stringFromID, stringFromSID } from "./ID"
import { listFromArray } from "./listFromArray"

function checkCtor(id: ID, forCtor: Ctor): boolean {
  return fromMaybe(bindMaybe(_get(id, ctorField.id), ctor => ctor === forCtor.id), () => false) }

function checkAlgebraicType<A>(id: ID, xs: {ctor: Ctor, f: (id: ID) => A}[]): Maybe<A> {
  return mapMaybe(_get(id, ctorField.id), _ctor => mapMaybe(xs.find(({ctor}) => ctor.id === _ctor), ({f}) => f(id))) }

export function checkString(id: ID): Maybe<HasSID> { return mapMaybe(sidFromID(id), sid => new HasSID(sid)) }
export function checkNumber(id: ID): Maybe<HasNID> { return mapMaybe(nidFromID(id), nid => new HasNID(nid)) }

function getList<A extends HasID>(_this: HasID, field: Field, f: (id: ID) => Maybe<A>) {
  return bindMaybe(_get(_this.id, field.id), x => mapMaybe(listFromID(x, f), arrayFromList)) }

function get<A>(_this: HasID, field: Field, f: (id: ID) => Maybe<A>) { return bindMaybe(_get(_this.id, field.id), f) }
function _set<A, B extends HasGUID>(_this: B, field: Field, f: (a: A) => ID, a: Maybe<A>) { setOrDelete(_this.id, field.id, mapMaybe(a, f)); return _this }
function setList<A extends HasID, B extends HasGUID>(_this: B, field: Field, f: (a: A) => ID, as: Maybe<A[]>) {
  setOrDelete(_this.id, field.id, mapMaybe(as, as => listFromArray<HasID>(as, id => ({id})).id)); return _this }

function getID(hasID: HasID) { return hasID.id }

export type HasID = { readonly id: ID }
export type HasGUID = { readonly id: GUID }

export class HasSID {
  constructor(public readonly id: SID) {}
  get string() { return stringFromSID(this.id) } }

export class HasNID {
  constructor(public readonly id: NID) {}
  get number() { return numberFromNID(this.id) } }

export class NonemptyList<A extends HasID = HasID> {
  constructor(public readonly id: ID, public f: (id: ID) => Maybe<A>) {}
  static fromID<A extends HasID>(id: ID, f: (id: ID) => Maybe<A>): Maybe<NonemptyList<A>> { return checkCtor(id, nonemptyListCtor) ? new NonemptyList(id, f) : nothing }
  get guidList() { return mapMaybe(guidFromID(this.id), guid => new GUIDNonemptyList(guid, this.f)) }
  get head() { return get(this, headField, this.f) }
  get tail() { return get(this, tailField, id => listFromID(id, this.f)) } }
export class GUIDNonemptyList<A extends HasID> extends NonemptyList<A> {
  constructor(public readonly id: GUID, public f: (id: ID) => Maybe<A>) { super(id, f) }
  static new<A extends HasID>(f: (id: ID) => Maybe<A>, guid: GUID = generateGUID()) { set(guid, ctorField.id, nonemptyListCtor.id); return new GUIDNonemptyList(guid, f) }
  get guidList() { return this }
  setHead(head: Maybe<A>) { return _set(this, headField, getID, head) }
  setTail(tail: Maybe<List<A>>) { return _set(this, tailField, getID, tail) } }

export class AlgebraicType {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, algebraicTypeCtor) ? new AlgebraicType(id) : nothing }
  get guidAlgebraicType() { return mapMaybe(guidFromID(this.id), guid => new GUIDAlgebraicType(guid)) }
  get name(): Maybe<string> { return get(this, nameField, stringFromID) }
  get ctorOrAlgebraicTypes(): Maybe<CtorOrAlgebraicType[]> { return getList(this, ctorOrAlgebraicTypesField, ctorOrAlgebraicTypeFromID) } }
export class GUIDAlgebraicType extends AlgebraicType {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, algebraicTypeCtor.id); return new GUIDAlgebraicType(guid) }
  get guidAlgebraicType() { return this }
  setName(x: Maybe<string>) { return _set(this, nameField, sidFromString, x) }
  setCtorOrAlgebraicTypes(x: Maybe<CtorOrAlgebraicType[]>) { return setList(this, ctorOrAlgebraicTypesField, getID, x) } }

export class And {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, andCtor) ? new And(id) : nothing }
  get guidAnd() { return mapMaybe(guidFromID(this.id), guid => new GUIDAnd(guid)) }
   }
export class GUIDAnd extends And {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, andCtor.id); return new GUIDAnd(guid) }
  get guidAnd() { return this }
   }

export class App {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, appCtor) ? new App(id) : nothing }
  get guidApp() { return mapMaybe(guidFromID(this.id), guid => new GUIDApp(guid)) }
  get name(): Maybe<string> { return get(this, nameField, stringFromID) } }
export class GUIDApp extends App {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, appCtor.id); return new GUIDApp(guid) }
  get guidApp() { return this }
  setName(x: Maybe<string>) { return _set(this, nameField, sidFromString, x) } }

export class AppPlatform {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, appPlatformCtor) ? new AppPlatform(id) : nothing }
  get guidAppPlatform() { return mapMaybe(guidFromID(this.id), guid => new GUIDAppPlatform(guid)) }
  get app(): Maybe<App> { return get(this, appField, App.fromID) }
  get platform(): Maybe<Platform> { return get(this, platformField, Platform.fromID) } }
export class GUIDAppPlatform extends AppPlatform {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, appPlatformCtor.id); return new GUIDAppPlatform(guid) }
  get guidAppPlatform() { return this }
  setApp(x: Maybe<App>) { return _set(this, appField, getID, x) }
  setPlatform(x: Maybe<Platform>) { return _set(this, platformField, getID, x) } }

export class ArrayLiteral {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, arrayLiteralCtor) ? new ArrayLiteral(id) : nothing }
  get guidArrayLiteral() { return mapMaybe(guidFromID(this.id), guid => new GUIDArrayLiteral(guid)) }
  get expressions(): Maybe<Expression[]> { return getList(this, expressionsField, expressionFromID) } }
export class GUIDArrayLiteral extends ArrayLiteral {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, arrayLiteralCtor.id); return new GUIDArrayLiteral(guid) }
  get guidArrayLiteral() { return this }
  setExpressions(x: Maybe<Expression[]>) { return setList(this, expressionsField, getID, x) } }

export class ArrowFunction {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, arrowFunctionCtor) ? new ArrowFunction(id) : nothing }
  get guidArrowFunction() { return mapMaybe(guidFromID(this.id), guid => new GUIDArrowFunction(guid)) }
  get parameters(): Maybe<Parameter[]> { return getList(this, parametersField, Parameter.fromID) }
  get statements(): Maybe<Statement[]> { return getList(this, statementsField, statementFromID) } }
export class GUIDArrowFunction extends ArrowFunction {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, arrowFunctionCtor.id); return new GUIDArrowFunction(guid) }
  get guidArrowFunction() { return this }
  setParameters(x: Maybe<Parameter[]>) { return setList(this, parametersField, getID, x) }
  setStatements(x: Maybe<Statement[]>) { return setList(this, statementsField, getID, x) } }

export class Assignment {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, assignmentCtor) ? new Assignment(id) : nothing }
  get guidAssignment() { return mapMaybe(guidFromID(this.id), guid => new GUIDAssignment(guid)) }
   }
export class GUIDAssignment extends Assignment {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, assignmentCtor.id); return new GUIDAssignment(guid) }
  get guidAssignment() { return this }
   }

export class AtomicType {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, atomicTypeCtor) ? new AtomicType(id) : nothing }
  get guidAtomicType() { return mapMaybe(guidFromID(this.id), guid => new GUIDAtomicType(guid)) }
  get name(): Maybe<string> { return get(this, nameField, stringFromID) } }
export class GUIDAtomicType extends AtomicType {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, atomicTypeCtor.id); return new GUIDAtomicType(guid) }
  get guidAtomicType() { return this }
  setName(x: Maybe<string>) { return _set(this, nameField, sidFromString, x) } }

export class AWSCredentials {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, awsCredentialsCtor) ? new AWSCredentials(id) : nothing }
  get guidAWSCredentials() { return mapMaybe(guidFromID(this.id), guid => new GUIDAWSCredentials(guid)) }
  get accessKeyId(): Maybe<string> { return get(this, accessKeyIdField, stringFromID) }
  get secretAccessKey(): Maybe<string> { return get(this, secretAccessKeyField, stringFromID) } }
export class GUIDAWSCredentials extends AWSCredentials {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, awsCredentialsCtor.id); return new GUIDAWSCredentials(guid) }
  get guidAWSCredentials() { return this }
  setAccessKeyId(x: Maybe<string>) { return _set(this, accessKeyIdField, sidFromString, x) }
  setSecretAccessKey(x: Maybe<string>) { return _set(this, secretAccessKeyField, sidFromString, x) } }

export class BinaryInline {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, binaryInlineCtor) ? new BinaryInline(id) : nothing }
  get guidBinaryInline() { return mapMaybe(guidFromID(this.id), guid => new GUIDBinaryInline(guid)) }
  get left(): Maybe<Expression> { return get(this, leftField, expressionFromID) }
  get binaryOperator(): Maybe<BinaryOperator> { return get(this, binaryOperatorField, binaryOperatorFromID) }
  get right(): Maybe<Expression> { return get(this, rightField, expressionFromID) } }
export class GUIDBinaryInline extends BinaryInline {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, binaryInlineCtor.id); return new GUIDBinaryInline(guid) }
  get guidBinaryInline() { return this }
  setLeft(x: Maybe<Expression>) { return _set(this, leftField, getID, x) }
  setBinaryOperator(x: Maybe<BinaryOperator>) { return _set(this, binaryOperatorField, getID, x) }
  setRight(x: Maybe<Expression>) { return _set(this, rightField, getID, x) } }

export class Block {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, blockCtor) ? new Block(id) : nothing }
  get guidBlock() { return mapMaybe(guidFromID(this.id), guid => new GUIDBlock(guid)) }
  get children(): Maybe<D[]> { return getList(this, childrenField, dFromID) } }
export class GUIDBlock extends Block {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, blockCtor.id); return new GUIDBlock(guid) }
  get guidBlock() { return this }
  setChildren(x: Maybe<D[]>) { return setList(this, childrenField, getID, x) } }

export class BradParams {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, bradParamsCtor) ? new BradParams(id) : nothing }
  get guidBradParams() { return mapMaybe(guidFromID(this.id), guid => new GUIDBradParams(guid)) }
  get adProbability(): Maybe<number> { return get(this, adProbabilityField, numberFromID) }
  get minimumCheckpointsPerAd(): Maybe<number> { return get(this, minimumCheckpointsPerAdField, numberFromID) }
  get timeIntervalPerAd(): Maybe<number> { return get(this, timeIntervalPerAdField, numberFromID) }
  get fetchPeriod(): Maybe<number> { return get(this, fetchPeriodField, numberFromID) }
  get tiers(): Maybe<List<WeightedEntry>[]> { return getList(this, tiersField, id => listFromID(id, weightedEntryFromID)) } }
export class GUIDBradParams extends BradParams {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, bradParamsCtor.id); return new GUIDBradParams(guid) }
  get guidBradParams() { return this }
  setAdProbability(x: Maybe<number>) { return _set(this, adProbabilityField, nidFromNumber, x) }
  setMinimumCheckpointsPerAd(x: Maybe<number>) { return _set(this, minimumCheckpointsPerAdField, nidFromNumber, x) }
  setTimeIntervalPerAd(x: Maybe<number>) { return _set(this, timeIntervalPerAdField, nidFromNumber, x) }
  setFetchPeriod(x: Maybe<number>) { return _set(this, fetchPeriodField, nidFromNumber, x) }
  setTiers(x: Maybe<List<WeightedEntry>[]>) { return setList(this, tiersField, getID, x) } }

export class Conditional {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, conditionalCtor) ? new Conditional(id) : nothing }
  get guidConditional() { return mapMaybe(guidFromID(this.id), guid => new GUIDConditional(guid)) }
  get condition(): Maybe<Expression> { return get(this, conditionField, expressionFromID) }
  get trueExpression(): Maybe<Expression> { return get(this, trueExpressionField, expressionFromID) }
  get falseExpression(): Maybe<Expression> { return get(this, falseExpressionField, expressionFromID) } }
export class GUIDConditional extends Conditional {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, conditionalCtor.id); return new GUIDConditional(guid) }
  get guidConditional() { return this }
  setCondition(x: Maybe<Expression>) { return _set(this, conditionField, getID, x) }
  setTrueExpression(x: Maybe<Expression>) { return _set(this, trueExpressionField, getID, x) }
  setFalseExpression(x: Maybe<Expression>) { return _set(this, falseExpressionField, getID, x) } }

export class Const {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, constCtor) ? new Const(id) : nothing }
  get guidConst() { return mapMaybe(guidFromID(this.id), guid => new GUIDConst(guid)) }
   }
export class GUIDConst extends Const {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, constCtor.id); return new GUIDConst(guid) }
  get guidConst() { return this }
   }

export class Ctor {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, ctorCtor) ? new Ctor(id) : nothing }
  get guidCtor() { return mapMaybe(guidFromID(this.id), guid => new GUIDCtor(guid)) }
  get name(): Maybe<string> { return get(this, nameField, stringFromID) }
  get fields(): Maybe<Field[]> { return getList(this, fieldsField, Field.fromID) } }
export class GUIDCtor extends Ctor {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, ctorCtor.id); return new GUIDCtor(guid) }
  get guidCtor() { return this }
  setName(x: Maybe<string>) { return _set(this, nameField, sidFromString, x) }
  setFields(x: Maybe<Field[]>) { return setList(this, fieldsField, getID, x) } }

export class CtorField {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, ctorFieldCtor) ? new CtorField(id) : nothing }
  get guidCtorField() { return mapMaybe(guidFromID(this.id), guid => new GUIDCtorField(guid)) }
  get ctor(): Maybe<Ctor> { return get(this, ctorField, Ctor.fromID) } }
export class GUIDCtorField extends CtorField {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, ctorFieldCtor.id); return new GUIDCtorField(guid) }
  get guidCtorField() { return this }
  setCtor(x: Maybe<Ctor>) { return _set(this, ctorField, getID, x) } }

export class Descend {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, descendCtor) ? new Descend(id) : nothing }
  get guidDescend() { return mapMaybe(guidFromID(this.id), guid => new GUIDDescend(guid)) }
  get field(): Maybe<Field> { return get(this, fieldField, Field.fromID) }
  get contextRender(): Maybe<Render> { return get(this, contextRenderField, renderFromID) } }
export class GUIDDescend extends Descend {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, descendCtor.id); return new GUIDDescend(guid) }
  get guidDescend() { return this }
  setField(x: Maybe<Field>) { return _set(this, fieldField, getID, x) }
  setContextRender(x: Maybe<Render>) { return _set(this, contextRenderField, getID, x) } }

export class Difference {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, differenceCtor) ? new Difference(id) : nothing }
  get guidDifference() { return mapMaybe(guidFromID(this.id), guid => new GUIDDifference(guid)) }
   }
export class GUIDDifference extends Difference {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, differenceCtor.id); return new GUIDDifference(guid) }
  get guidDifference() { return this }
   }

export class Dispatch {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, dispatchCtor) ? new Dispatch(id) : nothing }
  get guidDispatch() { return mapMaybe(guidFromID(this.id), guid => new GUIDDispatch(guid)) }
  get renders(): Maybe<Render[]> { return getList(this, rendersField, renderFromID) } }
export class GUIDDispatch extends Dispatch {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, dispatchCtor.id); return new GUIDDispatch(guid) }
  get guidDispatch() { return this }
  setRenders(x: Maybe<Render[]>) { return setList(this, rendersField, getID, x) } }

export class Dot {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, dotCtor) ? new Dot(id) : nothing }
  get guidDot() { return mapMaybe(guidFromID(this.id), guid => new GUIDDot(guid)) }
   }
export class GUIDDot extends Dot {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, dotCtor.id); return new GUIDDot(guid) }
  get guidDot() { return this }
   }

export class EmptyList {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, emptyListCtor) ? new EmptyList(id) : nothing }
  get guidEmptyList() { return mapMaybe(guidFromID(this.id), guid => new GUIDEmptyList(guid)) }
   }
export class GUIDEmptyList extends EmptyList {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, emptyListCtor.id); return new GUIDEmptyList(guid) }
  get guidEmptyList() { return this }
   }

export class Equals {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, equalsCtor) ? new Equals(id) : nothing }
  get guidEquals() { return mapMaybe(guidFromID(this.id), guid => new GUIDEquals(guid)) }
   }
export class GUIDEquals extends Equals {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, equalsCtor.id); return new GUIDEquals(guid) }
  get guidEquals() { return this }
   }

export class Evaluate {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, evaluateCtor) ? new Evaluate(id) : nothing }
  get guidEvaluate() { return mapMaybe(guidFromID(this.id), guid => new GUIDEvaluate(guid)) }
  get javascriptProgram(): Maybe<JavaScriptProgram> { return get(this, javascriptProgramField, JavaScriptProgram.fromID) } }
export class GUIDEvaluate extends Evaluate {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, evaluateCtor.id); return new GUIDEvaluate(guid) }
  get guidEvaluate() { return this }
  setJavascriptProgram(x: Maybe<JavaScriptProgram>) { return _set(this, javascriptProgramField, getID, x) } }

export class Extern {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, externCtor) ? new Extern(id) : nothing }
  get guidExtern() { return mapMaybe(guidFromID(this.id), guid => new GUIDExtern(guid)) }
  get name(): Maybe<string> { return get(this, nameField, stringFromID) } }
export class GUIDExtern extends Extern {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, externCtor.id); return new GUIDExtern(guid) }
  get guidExtern() { return this }
  setName(x: Maybe<string>) { return _set(this, nameField, sidFromString, x) } }

export class Field {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, fieldCtor) ? new Field(id) : nothing }
  get guidField() { return mapMaybe(guidFromID(this.id), guid => new GUIDField(guid)) }
  get name(): Maybe<string> { return get(this, nameField, stringFromID) }
  get type(): Maybe<Type> { return get(this, typeField, typeFromID) } }
export class GUIDField extends Field {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, fieldCtor.id); return new GUIDField(guid) }
  get guidField() { return this }
  setName(x: Maybe<string>) { return _set(this, nameField, sidFromString, x) }
  setType(x: Maybe<Type>) { return _set(this, typeField, getID, x) } }

export class FunctionCall {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, functionCallCtor) ? new FunctionCall(id) : nothing }
  get guidFunctionCall() { return mapMaybe(guidFromID(this.id), guid => new GUIDFunctionCall(guid)) }
  get function(): Maybe<Expression> { return get(this, functionField, expressionFromID) }
  get arguments(): Maybe<Expression[]> { return getList(this, argumentsField, expressionFromID) } }
export class GUIDFunctionCall extends FunctionCall {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, functionCallCtor.id); return new GUIDFunctionCall(guid) }
  get guidFunctionCall() { return this }
  setFunction(x: Maybe<Expression>) { return _set(this, functionField, getID, x) }
  setArguments(x: Maybe<Expression[]>) { return setList(this, argumentsField, getID, x) } }

export class FunctionDeclaration {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, functionDeclarationCtor) ? new FunctionDeclaration(id) : nothing }
  get guidFunctionDeclaration() { return mapMaybe(guidFromID(this.id), guid => new GUIDFunctionDeclaration(guid)) }
  get name(): Maybe<string> { return get(this, nameField, stringFromID) }
  get parameters(): Maybe<Parameter[]> { return getList(this, parametersField, Parameter.fromID) }
  get statements(): Maybe<Statement[]> { return getList(this, statementsField, statementFromID) } }
export class GUIDFunctionDeclaration extends FunctionDeclaration {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, functionDeclarationCtor.id); return new GUIDFunctionDeclaration(guid) }
  get guidFunctionDeclaration() { return this }
  setName(x: Maybe<string>) { return _set(this, nameField, sidFromString, x) }
  setParameters(x: Maybe<Parameter[]>) { return setList(this, parametersField, getID, x) }
  setStatements(x: Maybe<Statement[]>) { return setList(this, statementsField, getID, x) } }

export class GreaterThan {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, greaterThanCtor) ? new GreaterThan(id) : nothing }
  get guidGreaterThan() { return mapMaybe(guidFromID(this.id), guid => new GUIDGreaterThan(guid)) }
   }
export class GUIDGreaterThan extends GreaterThan {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, greaterThanCtor.id); return new GUIDGreaterThan(guid) }
  get guidGreaterThan() { return this }
   }

export class GreaterThanOrEqualTo {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, greaterThanOrEqualToCtor) ? new GreaterThanOrEqualTo(id) : nothing }
  get guidGreaterThanOrEqualTo() { return mapMaybe(guidFromID(this.id), guid => new GUIDGreaterThanOrEqualTo(guid)) }
   }
export class GUIDGreaterThanOrEqualTo extends GreaterThanOrEqualTo {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, greaterThanOrEqualToCtor.id); return new GUIDGreaterThanOrEqualTo(guid) }
  get guidGreaterThanOrEqualTo() { return this }
   }

export class HouseAdEntry {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, houseAdEntryCtor) ? new HouseAdEntry(id) : nothing }
  get guidHouseAdEntry() { return mapMaybe(guidFromID(this.id), guid => new GUIDHouseAdEntry(guid)) }
  get weight(): Maybe<number> { return get(this, weightField, numberFromID) }
  get lifetimeCap(): Maybe<number> { return get(this, lifetimeCapField, numberFromID) }
  get name(): Maybe<string> { return get(this, nameField, stringFromID) }
  get actionURL(): Maybe<string> { return get(this, actionURLField, stringFromID) }
  get images(): Maybe<HouseAdImage[]> { return getList(this, imagesField, HouseAdImage.fromID) } }
export class GUIDHouseAdEntry extends HouseAdEntry {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, houseAdEntryCtor.id); return new GUIDHouseAdEntry(guid) }
  get guidHouseAdEntry() { return this }
  setWeight(x: Maybe<number>) { return _set(this, weightField, nidFromNumber, x) }
  setLifetimeCap(x: Maybe<number>) { return _set(this, lifetimeCapField, nidFromNumber, x) }
  setName(x: Maybe<string>) { return _set(this, nameField, sidFromString, x) }
  setActionURL(x: Maybe<string>) { return _set(this, actionURLField, sidFromString, x) }
  setImages(x: Maybe<HouseAdImage[]>) { return setList(this, imagesField, getID, x) } }

export class HouseAdImage {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, houseAdImageCtor) ? new HouseAdImage(id) : nothing }
  get guidHouseAdImage() { return mapMaybe(guidFromID(this.id), guid => new GUIDHouseAdImage(guid)) }
  get width(): Maybe<number> { return get(this, widthField, numberFromID) }
  get height(): Maybe<number> { return get(this, heightField, numberFromID) }
  get extension(): Maybe<string> { return get(this, extensionField, stringFromID) }
  get sha1(): Maybe<string> { return get(this, sha1Field, stringFromID) } }
export class GUIDHouseAdImage extends HouseAdImage {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, houseAdImageCtor.id); return new GUIDHouseAdImage(guid) }
  get guidHouseAdImage() { return this }
  setWidth(x: Maybe<number>) { return _set(this, widthField, nidFromNumber, x) }
  setHeight(x: Maybe<number>) { return _set(this, heightField, nidFromNumber, x) }
  setExtension(x: Maybe<string>) { return _set(this, extensionField, sidFromString, x) }
  setSha1(x: Maybe<string>) { return _set(this, sha1Field, sidFromString, x) } }

export class If {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, ifCtor) ? new If(id) : nothing }
  get guidIf() { return mapMaybe(guidFromID(this.id), guid => new GUIDIf(guid)) }
  get condition(): Maybe<Expression> { return get(this, conditionField, expressionFromID) }
  get trueStatements(): Maybe<Statement[]> { return getList(this, trueStatementsField, statementFromID) }
  get falseStatements(): Maybe<Statement[]> { return getList(this, falseStatementsField, statementFromID) } }
export class GUIDIf extends If {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, ifCtor.id); return new GUIDIf(guid) }
  get guidIf() { return this }
  setCondition(x: Maybe<Expression>) { return _set(this, conditionField, getID, x) }
  setTrueStatements(x: Maybe<Statement[]>) { return setList(this, trueStatementsField, getID, x) }
  setFalseStatements(x: Maybe<Statement[]>) { return setList(this, falseStatementsField, getID, x) } }

export class JavaScriptProgram {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, javascriptProgramCtor) ? new JavaScriptProgram(id) : nothing }
  get guidJavaScriptProgram() { return mapMaybe(guidFromID(this.id), guid => new GUIDJavaScriptProgram(guid)) }
  get statements(): Maybe<Statement[]> { return getList(this, statementsField, statementFromID) } }
export class GUIDJavaScriptProgram extends JavaScriptProgram {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, javascriptProgramCtor.id); return new GUIDJavaScriptProgram(guid) }
  get guidJavaScriptProgram() { return this }
  setStatements(x: Maybe<Statement[]>) { return setList(this, statementsField, getID, x) } }

export class JSONArray {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, jsonArrayCtor) ? new JSONArray(id) : nothing }
  get guidJSONArray() { return mapMaybe(guidFromID(this.id), guid => new GUIDJSONArray(guid)) }
  get jsonArray(): Maybe<JSON[]> { return getList(this, jsonArrayField, jsonFromID) } }
export class GUIDJSONArray extends JSONArray {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, jsonArrayCtor.id); return new GUIDJSONArray(guid) }
  get guidJSONArray() { return this }
  setJsonArray(x: Maybe<JSON[]>) { return setList(this, jsonArrayField, getID, x) } }

export class JSONNumber {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, jsonNumberCtor) ? new JSONNumber(id) : nothing }
  get guidJSONNumber() { return mapMaybe(guidFromID(this.id), guid => new GUIDJSONNumber(guid)) }
  get jsonNumber(): Maybe<number> { return get(this, jsonNumberField, numberFromID) } }
export class GUIDJSONNumber extends JSONNumber {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, jsonNumberCtor.id); return new GUIDJSONNumber(guid) }
  get guidJSONNumber() { return this }
  setJsonNumber(x: Maybe<number>) { return _set(this, jsonNumberField, nidFromNumber, x) } }

export class JSONObject {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, jsonObjectCtor) ? new JSONObject(id) : nothing }
  get guidJSONObject() { return mapMaybe(guidFromID(this.id), guid => new GUIDJSONObject(guid)) }
  get keyValuePairs(): Maybe<KeyValuePair[]> { return getList(this, keyValuePairsField, KeyValuePair.fromID) } }
export class GUIDJSONObject extends JSONObject {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, jsonObjectCtor.id); return new GUIDJSONObject(guid) }
  get guidJSONObject() { return this }
  setKeyValuePairs(x: Maybe<KeyValuePair[]>) { return setList(this, keyValuePairsField, getID, x) } }

export class JSONString {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, jsonStringCtor) ? new JSONString(id) : nothing }
  get guidJSONString() { return mapMaybe(guidFromID(this.id), guid => new GUIDJSONString(guid)) }
  get jsonString(): Maybe<string> { return get(this, jsonStringField, stringFromID) } }
export class GUIDJSONString extends JSONString {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, jsonStringCtor.id); return new GUIDJSONString(guid) }
  get guidJSONString() { return this }
  setJsonString(x: Maybe<string>) { return _set(this, jsonStringField, sidFromString, x) } }

export class KeyValue {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, keyValueCtor) ? new KeyValue(id) : nothing }
  get guidKeyValue() { return mapMaybe(guidFromID(this.id), guid => new GUIDKeyValue(guid)) }
  get objectKey(): Maybe<string> { return get(this, objectKeyField, stringFromID) }
  get objectValue(): Maybe<Expression> { return get(this, objectValueField, expressionFromID) } }
export class GUIDKeyValue extends KeyValue {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, keyValueCtor.id); return new GUIDKeyValue(guid) }
  get guidKeyValue() { return this }
  setObjectKey(x: Maybe<string>) { return _set(this, objectKeyField, sidFromString, x) }
  setObjectValue(x: Maybe<Expression>) { return _set(this, objectValueField, getID, x) } }

export class KeyValuePair {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, keyValuePairCtor) ? new KeyValuePair(id) : nothing }
  get guidKeyValuePair() { return mapMaybe(guidFromID(this.id), guid => new GUIDKeyValuePair(guid)) }
  get key(): Maybe<string> { return get(this, keyField, stringFromID) }
  get value(): Maybe<JSON> { return get(this, valueField, jsonFromID) } }
export class GUIDKeyValuePair extends KeyValuePair {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, keyValuePairCtor.id); return new GUIDKeyValuePair(guid) }
  get guidKeyValuePair() { return this }
  setKey(x: Maybe<string>) { return _set(this, keyField, sidFromString, x) }
  setValue(x: Maybe<JSON>) { return _set(this, valueField, getID, x) } }

export class Label {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, labelCtor) ? new Label(id) : nothing }
  get guidLabel() { return mapMaybe(guidFromID(this.id), guid => new GUIDLabel(guid)) }
  get field(): Maybe<Field> { return get(this, fieldField, Field.fromID) }
  get child(): Maybe<D> { return get(this, childField, dFromID) } }
export class GUIDLabel extends Label {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, labelCtor.id); return new GUIDLabel(guid) }
  get guidLabel() { return this }
  setField(x: Maybe<Field>) { return _set(this, fieldField, getID, x) }
  setChild(x: Maybe<D>) { return _set(this, childField, getID, x) } }

export class LessThan {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, lessThanCtor) ? new LessThan(id) : nothing }
  get guidLessThan() { return mapMaybe(guidFromID(this.id), guid => new GUIDLessThan(guid)) }
   }
export class GUIDLessThan extends LessThan {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, lessThanCtor.id); return new GUIDLessThan(guid) }
  get guidLessThan() { return this }
   }

export class LessThanOrEqualTo {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, lessThanOrEqualToCtor) ? new LessThanOrEqualTo(id) : nothing }
  get guidLessThanOrEqualTo() { return mapMaybe(guidFromID(this.id), guid => new GUIDLessThanOrEqualTo(guid)) }
   }
export class GUIDLessThanOrEqualTo extends LessThanOrEqualTo {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, lessThanOrEqualToCtor.id); return new GUIDLessThanOrEqualTo(guid) }
  get guidLessThanOrEqualTo() { return this }
   }

export class Let {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, letCtor) ? new Let(id) : nothing }
  get guidLet() { return mapMaybe(guidFromID(this.id), guid => new GUIDLet(guid)) }
   }
export class GUIDLet extends Let {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, letCtor.id); return new GUIDLet(guid) }
  get guidLet() { return this }
   }

export class Line {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, lineCtor) ? new Line(id) : nothing }
  get guidLine() { return mapMaybe(guidFromID(this.id), guid => new GUIDLine(guid)) }
  get children(): Maybe<D[]> { return getList(this, childrenField, dFromID) } }
export class GUIDLine extends Line {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, lineCtor.id); return new GUIDLine(guid) }
  get guidLine() { return this }
  setChildren(x: Maybe<D[]>) { return setList(this, childrenField, getID, x) } }

export class ListType {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, listTypeCtor) ? new ListType(id) : nothing }
  get guidListType() { return mapMaybe(guidFromID(this.id), guid => new GUIDListType(guid)) }
  get type(): Maybe<Type> { return get(this, typeField, typeFromID) } }
export class GUIDListType extends ListType {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, listTypeCtor.id); return new GUIDListType(guid) }
  get guidListType() { return this }
  setType(x: Maybe<Type>) { return _set(this, typeField, getID, x) } }

export class LoadAWS {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, loadAWSCtor) ? new LoadAWS(id) : nothing }
  get guidLoadAWS() { return mapMaybe(guidFromID(this.id), guid => new GUIDLoadAWS(guid)) }
  get bucket(): Maybe<string> { return get(this, bucketField, stringFromID) }
  get credentials(): Maybe<AWSCredentials> { return get(this, credentialsField, AWSCredentials.fromID) }
  get key(): Maybe<string> { return get(this, keyField, stringFromID) } }
export class GUIDLoadAWS extends LoadAWS {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, loadAWSCtor.id); return new GUIDLoadAWS(guid) }
  get guidLoadAWS() { return this }
  setBucket(x: Maybe<string>) { return _set(this, bucketField, sidFromString, x) }
  setCredentials(x: Maybe<AWSCredentials>) { return _set(this, credentialsField, getID, x) }
  setKey(x: Maybe<string>) { return _set(this, keyField, sidFromString, x) } }

export class LoadJSON {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, loadJSONCtor) ? new LoadJSON(id) : nothing }
  get guidLoadJSON() { return mapMaybe(guidFromID(this.id), guid => new GUIDLoadJSON(guid)) }
  get url(): Maybe<string> { return get(this, urlField, stringFromID) } }
export class GUIDLoadJSON extends LoadJSON {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, loadJSONCtor.id); return new GUIDLoadJSON(guid) }
  get guidLoadJSON() { return this }
  setUrl(x: Maybe<string>) { return _set(this, urlField, sidFromString, x) } }

export class Module {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, moduleCtor) ? new Module(id) : nothing }
  get guidModule() { return mapMaybe(guidFromID(this.id), guid => new GUIDModule(guid)) }
  get ctorOrAlgebraicTypes(): Maybe<CtorOrAlgebraicType[]> { return getList(this, ctorOrAlgebraicTypesField, ctorOrAlgebraicTypeFromID) }
  get data(): Maybe<HasID> { return get(this, dataField, id => ({id})) }
  get renderCtors(): Maybe<RenderCtor[]> { return getList(this, renderCtorsField, RenderCtor.fromID) }
  get transformations(): Maybe<HasID[]> { return getList(this, transformationsField, id => ({id})) } }
export class GUIDModule extends Module {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, moduleCtor.id); return new GUIDModule(guid) }
  get guidModule() { return this }
  setCtorOrAlgebraicTypes(x: Maybe<CtorOrAlgebraicType[]>) { return setList(this, ctorOrAlgebraicTypesField, getID, x) }
  setData(x: Maybe<HasID>) { return _set(this, dataField, getID, x) }
  setRenderCtors(x: Maybe<RenderCtor[]>) { return setList(this, renderCtorsField, getID, x) }
  setTransformations(x: Maybe<HasID[]>) { return setList(this, transformationsField, getID, x) } }

export class NetworkEntry {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, networkEntryCtor) ? new NetworkEntry(id) : nothing }
  get guidNetworkEntry() { return mapMaybe(guidFromID(this.id), guid => new GUIDNetworkEntry(guid)) }
  get weight(): Maybe<number> { return get(this, weightField, numberFromID) }
  get lifetimeCap(): Maybe<number> { return get(this, lifetimeCapField, numberFromID) }
  get name(): Maybe<string> { return get(this, nameField, stringFromID) } }
export class GUIDNetworkEntry extends NetworkEntry {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, networkEntryCtor.id); return new GUIDNetworkEntry(guid) }
  get guidNetworkEntry() { return this }
  setWeight(x: Maybe<number>) { return _set(this, weightField, nidFromNumber, x) }
  setLifetimeCap(x: Maybe<number>) { return _set(this, lifetimeCapField, nidFromNumber, x) }
  setName(x: Maybe<string>) { return _set(this, nameField, sidFromString, x) } }

export class New {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, newCtor) ? new New(id) : nothing }
  get guidNew() { return mapMaybe(guidFromID(this.id), guid => new GUIDNew(guid)) }
  get expression(): Maybe<Expression> { return get(this, expressionField, expressionFromID) }
  get arguments(): Maybe<Expression[]> { return getList(this, argumentsField, expressionFromID) } }
export class GUIDNew extends New {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, newCtor.id); return new GUIDNew(guid) }
  get guidNew() { return this }
  setExpression(x: Maybe<Expression>) { return _set(this, expressionField, getID, x) }
  setArguments(x: Maybe<Expression[]>) { return setList(this, argumentsField, getID, x) } }

export class NotEquals {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, notEqualsCtor) ? new NotEquals(id) : nothing }
  get guidNotEquals() { return mapMaybe(guidFromID(this.id), guid => new GUIDNotEquals(guid)) }
   }
export class GUIDNotEquals extends NotEquals {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, notEqualsCtor.id); return new GUIDNotEquals(guid) }
  get guidNotEquals() { return this }
   }

export class Null {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, nullCtor) ? new Null(id) : nothing }
  get guidNull() { return mapMaybe(guidFromID(this.id), guid => new GUIDNull(guid)) }
   }
export class GUIDNull extends Null {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, nullCtor.id); return new GUIDNull(guid) }
  get guidNull() { return this }
   }

export class ObjectLiteral {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, objectLiteralCtor) ? new ObjectLiteral(id) : nothing }
  get guidObjectLiteral() { return mapMaybe(guidFromID(this.id), guid => new GUIDObjectLiteral(guid)) }
  get keyValues(): Maybe<KeyValue[]> { return getList(this, keyValuesField, KeyValue.fromID) } }
export class GUIDObjectLiteral extends ObjectLiteral {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, objectLiteralCtor.id); return new GUIDObjectLiteral(guid) }
  get guidObjectLiteral() { return this }
  setKeyValues(x: Maybe<KeyValue[]>) { return setList(this, keyValuesField, getID, x) } }

export class Or {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, orCtor) ? new Or(id) : nothing }
  get guidOr() { return mapMaybe(guidFromID(this.id), guid => new GUIDOr(guid)) }
   }
export class GUIDOr extends Or {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, orCtor.id); return new GUIDOr(guid) }
  get guidOr() { return this }
   }

export class Parameter {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, parameterCtor) ? new Parameter(id) : nothing }
  get guidParameter() { return mapMaybe(guidFromID(this.id), guid => new GUIDParameter(guid)) }
  get name(): Maybe<string> { return get(this, nameField, stringFromID) } }
export class GUIDParameter extends Parameter {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, parameterCtor.id); return new GUIDParameter(guid) }
  get guidParameter() { return this }
  setName(x: Maybe<string>) { return _set(this, nameField, sidFromString, x) } }

export class Platform {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, platformCtor) ? new Platform(id) : nothing }
  get guidPlatform() { return mapMaybe(guidFromID(this.id), guid => new GUIDPlatform(guid)) }
  get name(): Maybe<string> { return get(this, nameField, stringFromID) } }
export class GUIDPlatform extends Platform {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, platformCtor.id); return new GUIDPlatform(guid) }
  get guidPlatform() { return this }
  setName(x: Maybe<string>) { return _set(this, nameField, sidFromString, x) } }

export class Product {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, productCtor) ? new Product(id) : nothing }
  get guidProduct() { return mapMaybe(guidFromID(this.id), guid => new GUIDProduct(guid)) }
   }
export class GUIDProduct extends Product {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, productCtor.id); return new GUIDProduct(guid) }
  get guidProduct() { return this }
   }

export class PutAWS {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, putAWSCtor) ? new PutAWS(id) : nothing }
  get guidPutAWS() { return mapMaybe(guidFromID(this.id), guid => new GUIDPutAWS(guid)) }
  get bucket(): Maybe<string> { return get(this, bucketField, stringFromID) }
  get credentials(): Maybe<AWSCredentials> { return get(this, credentialsField, AWSCredentials.fromID) }
  get key(): Maybe<string> { return get(this, keyField, stringFromID) }
  get string(): Maybe<string> { return get(this, stringField, stringFromID) } }
export class GUIDPutAWS extends PutAWS {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, putAWSCtor.id); return new GUIDPutAWS(guid) }
  get guidPutAWS() { return this }
  setBucket(x: Maybe<string>) { return _set(this, bucketField, sidFromString, x) }
  setCredentials(x: Maybe<AWSCredentials>) { return _set(this, credentialsField, getID, x) }
  setKey(x: Maybe<string>) { return _set(this, keyField, sidFromString, x) }
  setString(x: Maybe<string>) { return _set(this, stringField, sidFromString, x) } }

export class PutAWSSucceeded {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, putAWSSucceededCtor) ? new PutAWSSucceeded(id) : nothing }
  get guidPutAWSSucceeded() { return mapMaybe(guidFromID(this.id), guid => new GUIDPutAWSSucceeded(guid)) }
   }
export class GUIDPutAWSSucceeded extends PutAWSSucceeded {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, putAWSSucceededCtor.id); return new GUIDPutAWSSucceeded(guid) }
  get guidPutAWSSucceeded() { return this }
   }

export class Quotient {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, quotientCtor) ? new Quotient(id) : nothing }
  get guidQuotient() { return mapMaybe(guidFromID(this.id), guid => new GUIDQuotient(guid)) }
   }
export class GUIDQuotient extends Quotient {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, quotientCtor.id); return new GUIDQuotient(guid) }
  get guidQuotient() { return this }
   }

export class RenderCtor {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, renderCtorCtor) ? new RenderCtor(id) : nothing }
  get guidRenderCtor() { return mapMaybe(guidFromID(this.id), guid => new GUIDRenderCtor(guid)) }
  get forCtor(): Maybe<Ctor> { return get(this, forCtorField, Ctor.fromID) }
  get d(): Maybe<D> { return get(this, dField, dFromID) } }
export class GUIDRenderCtor extends RenderCtor {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, renderCtorCtor.id); return new GUIDRenderCtor(guid) }
  get guidRenderCtor() { return this }
  setForCtor(x: Maybe<Ctor>) { return _set(this, forCtorField, getID, x) }
  setD(x: Maybe<D>) { return _set(this, dField, getID, x) } }

export class RenderList {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, renderListCtor) ? new RenderList(id) : nothing }
  get guidRenderList() { return mapMaybe(guidFromID(this.id), guid => new GUIDRenderList(guid)) }
  get opening(): Maybe<string> { return get(this, openingField, stringFromID) }
  get closing(): Maybe<string> { return get(this, closingField, stringFromID) }
  get separator(): Maybe<string> { return get(this, separatorField, stringFromID) }
  get contextRender(): Maybe<Render> { return get(this, contextRenderField, renderFromID) } }
export class GUIDRenderList extends RenderList {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, renderListCtor.id); return new GUIDRenderList(guid) }
  get guidRenderList() { return this }
  setOpening(x: Maybe<string>) { return _set(this, openingField, sidFromString, x) }
  setClosing(x: Maybe<string>) { return _set(this, closingField, sidFromString, x) }
  setSeparator(x: Maybe<string>) { return _set(this, separatorField, sidFromString, x) }
  setContextRender(x: Maybe<Render>) { return _set(this, contextRenderField, getID, x) } }

export class RenderNameShallow {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, renderNameShallowCtor) ? new RenderNameShallow(id) : nothing }
  get guidRenderNameShallow() { return mapMaybe(guidFromID(this.id), guid => new GUIDRenderNameShallow(guid)) }
  get forCtor(): Maybe<Ctor> { return get(this, forCtorField, Ctor.fromID) } }
export class GUIDRenderNameShallow extends RenderNameShallow {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, renderNameShallowCtor.id); return new GUIDRenderNameShallow(guid) }
  get guidRenderNameShallow() { return this }
  setForCtor(x: Maybe<Ctor>) { return _set(this, forCtorField, getID, x) } }

export class Return {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, returnCtor) ? new Return(id) : nothing }
  get guidReturn() { return mapMaybe(guidFromID(this.id), guid => new GUIDReturn(guid)) }
  get expression(): Maybe<Expression> { return get(this, expressionField, expressionFromID) } }
export class GUIDReturn extends Return {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, returnCtor.id); return new GUIDReturn(guid) }
  get guidReturn() { return this }
  setExpression(x: Maybe<Expression>) { return _set(this, expressionField, getID, x) } }

export class RootViews {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, rootViewsCtor) ? new RootViews(id) : nothing }
  get guidRootViews() { return mapMaybe(guidFromID(this.id), guid => new GUIDRootViews(guid)) }
  get root(): Maybe<HasID> { return get(this, rootField, id => ({id})) }
  get views(): Maybe<HasID[]> { return getList(this, viewsField, id => ({id})) } }
export class GUIDRootViews extends RootViews {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, rootViewsCtor.id); return new GUIDRootViews(guid) }
  get guidRootViews() { return this }
  setRoot(x: Maybe<HasID>) { return _set(this, rootField, getID, x) }
  setViews(x: Maybe<HasID[]>) { return setList(this, viewsField, getID, x) } }

export class StrictEquals {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, strictEqualsCtor) ? new StrictEquals(id) : nothing }
  get guidStrictEquals() { return mapMaybe(guidFromID(this.id), guid => new GUIDStrictEquals(guid)) }
   }
export class GUIDStrictEquals extends StrictEquals {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, strictEqualsCtor.id); return new GUIDStrictEquals(guid) }
  get guidStrictEquals() { return this }
   }

export class StrictNotEquals {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, strictNotEqualsCtor) ? new StrictNotEquals(id) : nothing }
  get guidStrictNotEquals() { return mapMaybe(guidFromID(this.id), guid => new GUIDStrictNotEquals(guid)) }
   }
export class GUIDStrictNotEquals extends StrictNotEquals {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, strictNotEqualsCtor.id); return new GUIDStrictNotEquals(guid) }
  get guidStrictNotEquals() { return this }
   }

export class Sum {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, sumCtor) ? new Sum(id) : nothing }
  get guidSum() { return mapMaybe(guidFromID(this.id), guid => new GUIDSum(guid)) }
   }
export class GUIDSum extends Sum {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, sumCtor.id); return new GUIDSum(guid) }
  get guidSum() { return this }
   }

export class Undefined {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, undefinedCtor) ? new Undefined(id) : nothing }
  get guidUndefined() { return mapMaybe(guidFromID(this.id), guid => new GUIDUndefined(guid)) }
   }
export class GUIDUndefined extends Undefined {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, undefinedCtor.id); return new GUIDUndefined(guid) }
  get guidUndefined() { return this }
   }

export class Var {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, varCtor) ? new Var(id) : nothing }
  get guidVar() { return mapMaybe(guidFromID(this.id), guid => new GUIDVar(guid)) }
   }
export class GUIDVar extends Var {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, varCtor.id); return new GUIDVar(guid) }
  get guidVar() { return this }
   }

export class VariableDeclaration {
  constructor(public readonly id: ID) {}
  static fromID(id: ID) { return checkCtor(id, variableDeclarationCtor) ? new VariableDeclaration(id) : nothing }
  get guidVariableDeclaration() { return mapMaybe(guidFromID(this.id), guid => new GUIDVariableDeclaration(guid)) }
  get name(): Maybe<string> { return get(this, nameField, stringFromID) }
  get constLetVar(): Maybe<ConstLetVar> { return get(this, constLetVarField, constLetVarFromID) }
  get expression(): Maybe<Expression> { return get(this, expressionField, expressionFromID) } }
export class GUIDVariableDeclaration extends VariableDeclaration {
  constructor(public readonly id: GUID) { super(id) }
  static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, variableDeclarationCtor.id); return new GUIDVariableDeclaration(guid) }
  get guidVariableDeclaration() { return this }
  setName(x: Maybe<string>) { return _set(this, nameField, sidFromString, x) }
  setConstLetVar(x: Maybe<ConstLetVar>) { return _set(this, constLetVarField, getID, x) }
  setExpression(x: Maybe<Expression>) { return _set(this, expressionField, getID, x) } }

export type List<A extends HasID = HasID> = NonemptyList<A> | EmptyList
export type GUIDList<A extends HasID = HasID> = GUIDNonemptyList<A> | GUIDEmptyList
export function listFromID<A extends HasID>(id: ID, f: (id: ID) => Maybe<A>): Maybe<List<A>> { return checkAlgebraicType<List<A>>(id, [{ctor: nonemptyListCtor, f: id => new NonemptyList(id, f)}, {ctor: emptyListCtor, f: id => new EmptyList(id)}]) }
export function matchList<A extends HasID, B>(x: List<A>, nonemptyListF: (x: NonemptyList<A>) => B, emptyListF: (x: EmptyList) => B) { return x instanceof NonemptyList ? nonemptyListF(x) : emptyListF(x) }
export function nonemptyListFromList<A extends HasID>(x: List<A>) { return x instanceof NonemptyList ? x : nothing }
export function emptyListFromList<A extends HasID>(x: List<A>) { return x instanceof EmptyList ? x : nothing }

export type BinaryOperator = Sum | Product | Quotient | Difference | And | Or | Dot | Equals | NotEquals | StrictEquals | StrictNotEquals | GreaterThan | LessThan | GreaterThanOrEqualTo | LessThanOrEqualTo | Assignment
export type GUIDBinaryOperator = GUIDSum | GUIDProduct | GUIDQuotient | GUIDDifference | GUIDAnd | GUIDOr | GUIDDot | GUIDEquals | GUIDNotEquals | GUIDStrictEquals | GUIDStrictNotEquals | GUIDGreaterThan | GUIDLessThan | GUIDGreaterThanOrEqualTo | GUIDLessThanOrEqualTo | GUIDAssignment
export function binaryOperatorFromID(id: ID): Maybe<BinaryOperator> { return checkAlgebraicType<BinaryOperator>(id, [{ctor: sumCtor, f: id => new Sum(id)}, {ctor: productCtor, f: id => new Product(id)}, {ctor: quotientCtor, f: id => new Quotient(id)}, {ctor: differenceCtor, f: id => new Difference(id)}, {ctor: andCtor, f: id => new And(id)}, {ctor: orCtor, f: id => new Or(id)}, {ctor: dotCtor, f: id => new Dot(id)}, {ctor: equalsCtor, f: id => new Equals(id)}, {ctor: notEqualsCtor, f: id => new NotEquals(id)}, {ctor: strictEqualsCtor, f: id => new StrictEquals(id)}, {ctor: strictNotEqualsCtor, f: id => new StrictNotEquals(id)}, {ctor: greaterThanCtor, f: id => new GreaterThan(id)}, {ctor: lessThanCtor, f: id => new LessThan(id)}, {ctor: greaterThanOrEqualToCtor, f: id => new GreaterThanOrEqualTo(id)}, {ctor: lessThanOrEqualToCtor, f: id => new LessThanOrEqualTo(id)}, {ctor: assignmentCtor, f: id => new Assignment(id)}]) }
export function matchBinaryOperator<A>(x: BinaryOperator, sumF: (x: Sum) => A, productF: (x: Product) => A, quotientF: (x: Quotient) => A, differenceF: (x: Difference) => A, andF: (x: And) => A, orF: (x: Or) => A, dotF: (x: Dot) => A, equalsF: (x: Equals) => A, notEqualsF: (x: NotEquals) => A, strictEqualsF: (x: StrictEquals) => A, strictNotEqualsF: (x: StrictNotEquals) => A, greaterThanF: (x: GreaterThan) => A, lessThanF: (x: LessThan) => A, greaterThanOrEqualToF: (x: GreaterThanOrEqualTo) => A, lessThanOrEqualToF: (x: LessThanOrEqualTo) => A, assignmentF: (x: Assignment) => A) { return x instanceof Sum ? sumF(x) : x instanceof Product ? productF(x) : x instanceof Quotient ? quotientF(x) : x instanceof Difference ? differenceF(x) : x instanceof And ? andF(x) : x instanceof Or ? orF(x) : x instanceof Dot ? dotF(x) : x instanceof Equals ? equalsF(x) : x instanceof NotEquals ? notEqualsF(x) : x instanceof StrictEquals ? strictEqualsF(x) : x instanceof StrictNotEquals ? strictNotEqualsF(x) : x instanceof GreaterThan ? greaterThanF(x) : x instanceof LessThan ? lessThanF(x) : x instanceof GreaterThanOrEqualTo ? greaterThanOrEqualToF(x) : x instanceof LessThanOrEqualTo ? lessThanOrEqualToF(x) : assignmentF(x) }
export function sumFromBinaryOperator(x: BinaryOperator) { return x instanceof Sum ? x : nothing }
export function productFromBinaryOperator(x: BinaryOperator) { return x instanceof Product ? x : nothing }
export function quotientFromBinaryOperator(x: BinaryOperator) { return x instanceof Quotient ? x : nothing }
export function differenceFromBinaryOperator(x: BinaryOperator) { return x instanceof Difference ? x : nothing }
export function andFromBinaryOperator(x: BinaryOperator) { return x instanceof And ? x : nothing }
export function orFromBinaryOperator(x: BinaryOperator) { return x instanceof Or ? x : nothing }
export function dotFromBinaryOperator(x: BinaryOperator) { return x instanceof Dot ? x : nothing }
export function equalsFromBinaryOperator(x: BinaryOperator) { return x instanceof Equals ? x : nothing }
export function notEqualsFromBinaryOperator(x: BinaryOperator) { return x instanceof NotEquals ? x : nothing }
export function strictEqualsFromBinaryOperator(x: BinaryOperator) { return x instanceof StrictEquals ? x : nothing }
export function strictNotEqualsFromBinaryOperator(x: BinaryOperator) { return x instanceof StrictNotEquals ? x : nothing }
export function greaterThanFromBinaryOperator(x: BinaryOperator) { return x instanceof GreaterThan ? x : nothing }
export function lessThanFromBinaryOperator(x: BinaryOperator) { return x instanceof LessThan ? x : nothing }
export function greaterThanOrEqualToFromBinaryOperator(x: BinaryOperator) { return x instanceof GreaterThanOrEqualTo ? x : nothing }
export function lessThanOrEqualToFromBinaryOperator(x: BinaryOperator) { return x instanceof LessThanOrEqualTo ? x : nothing }
export function assignmentFromBinaryOperator(x: BinaryOperator) { return x instanceof Assignment ? x : nothing }

export type ConstLetVar = Const | Let | Var
export type GUIDConstLetVar = GUIDConst | GUIDLet | GUIDVar
export function constLetVarFromID(id: ID): Maybe<ConstLetVar> { return checkAlgebraicType<ConstLetVar>(id, [{ctor: constCtor, f: id => new Const(id)}, {ctor: letCtor, f: id => new Let(id)}, {ctor: varCtor, f: id => new Var(id)}]) }
export function matchConstLetVar<A>(x: ConstLetVar, constF: (x: Const) => A, letF: (x: Let) => A, varF: (x: Var) => A) { return x instanceof Const ? constF(x) : x instanceof Let ? letF(x) : varF(x) }
export function constFromConstLetVar(x: ConstLetVar) { return x instanceof Const ? x : nothing }
export function letFromConstLetVar(x: ConstLetVar) { return x instanceof Let ? x : nothing }
export function varFromConstLetVar(x: ConstLetVar) { return x instanceof Var ? x : nothing }

export type CtorOrAlgebraicType = Ctor | AlgebraicType | AtomicType
export type GUIDCtorOrAlgebraicType = GUIDCtor | GUIDAlgebraicType | GUIDAtomicType
export function ctorOrAlgebraicTypeFromID(id: ID): Maybe<CtorOrAlgebraicType> { return checkAlgebraicType<CtorOrAlgebraicType>(id, [{ctor: ctorCtor, f: id => new Ctor(id)}, {ctor: algebraicTypeCtor, f: id => new AlgebraicType(id)}, {ctor: atomicTypeCtor, f: id => new AtomicType(id)}]) }
export function matchCtorOrAlgebraicType<A>(x: CtorOrAlgebraicType, ctorF: (x: Ctor) => A, algebraicTypeF: (x: AlgebraicType) => A, atomicTypeF: (x: AtomicType) => A) { return x instanceof Ctor ? ctorF(x) : x instanceof AlgebraicType ? algebraicTypeF(x) : atomicTypeF(x) }
export function ctorFromCtorOrAlgebraicType(x: CtorOrAlgebraicType) { return x instanceof Ctor ? x : nothing }
export function algebraicTypeFromCtorOrAlgebraicType(x: CtorOrAlgebraicType) { return x instanceof AlgebraicType ? x : nothing }
export function atomicTypeFromCtorOrAlgebraicType(x: CtorOrAlgebraicType) { return x instanceof AtomicType ? x : nothing }

export type D = Block | Line | Descend | Label | HasSID
export function dFromID(id: ID): Maybe<D> { return altMaybe(checkAlgebraicType<D>(id, [{ctor: blockCtor, f: id => new Block(id)}, {ctor: lineCtor, f: id => new Line(id)}, {ctor: descendCtor, f: id => new Descend(id)}, {ctor: labelCtor, f: id => new Label(id)}]), () => checkString(id)) }
export function matchD<A>(x: D, blockF: (x: Block) => A, lineF: (x: Line) => A, descendF: (x: Descend) => A, labelF: (x: Label) => A, stringF: (x: string) => A) { return x instanceof Block ? blockF(x) : x instanceof Line ? lineF(x) : x instanceof Descend ? descendF(x) : x instanceof Label ? labelF(x) : stringF(x.string) }
export function blockFromD(x: D) { return x instanceof Block ? x : nothing }
export function lineFromD(x: D) { return x instanceof Line ? x : nothing }
export function descendFromD(x: D) { return x instanceof Descend ? x : nothing }
export function labelFromD(x: D) { return x instanceof Label ? x : nothing }
export function stringFromD(x: D) { return stringFromID(x.id) }

export type Expression = FunctionDeclaration | Extern | Parameter | ArrayLiteral | ObjectLiteral | KeyValue | BinaryInline | Conditional | FunctionCall | ArrowFunction | New | Undefined | Null | HasNID | HasSID
export function expressionFromID(id: ID): Maybe<Expression> { return altMaybe(checkAlgebraicType<Expression>(id, [{ctor: functionDeclarationCtor, f: id => new FunctionDeclaration(id)}, {ctor: externCtor, f: id => new Extern(id)}, {ctor: parameterCtor, f: id => new Parameter(id)}, {ctor: arrayLiteralCtor, f: id => new ArrayLiteral(id)}, {ctor: objectLiteralCtor, f: id => new ObjectLiteral(id)}, {ctor: keyValueCtor, f: id => new KeyValue(id)}, {ctor: binaryInlineCtor, f: id => new BinaryInline(id)}, {ctor: conditionalCtor, f: id => new Conditional(id)}, {ctor: functionCallCtor, f: id => new FunctionCall(id)}, {ctor: arrowFunctionCtor, f: id => new ArrowFunction(id)}, {ctor: newCtor, f: id => new New(id)}, {ctor: undefinedCtor, f: id => new Undefined(id)}, {ctor: nullCtor, f: id => new Null(id)}]), () => checkNumber(id), () => checkString(id)) }
export function matchExpression<A>(x: Expression, functionDeclarationF: (x: FunctionDeclaration) => A, externF: (x: Extern) => A, parameterF: (x: Parameter) => A, arrayLiteralF: (x: ArrayLiteral) => A, objectLiteralF: (x: ObjectLiteral) => A, keyValueF: (x: KeyValue) => A, binaryInlineF: (x: BinaryInline) => A, conditionalF: (x: Conditional) => A, functionCallF: (x: FunctionCall) => A, arrowFunctionF: (x: ArrowFunction) => A, newF: (x: New) => A, undefinedF: (x: Undefined) => A, nullF: (x: Null) => A, numberF: (x: number) => A, stringF: (x: string) => A) { return x instanceof FunctionDeclaration ? functionDeclarationF(x) : x instanceof Extern ? externF(x) : x instanceof Parameter ? parameterF(x) : x instanceof ArrayLiteral ? arrayLiteralF(x) : x instanceof ObjectLiteral ? objectLiteralF(x) : x instanceof KeyValue ? keyValueF(x) : x instanceof BinaryInline ? binaryInlineF(x) : x instanceof Conditional ? conditionalF(x) : x instanceof FunctionCall ? functionCallF(x) : x instanceof ArrowFunction ? arrowFunctionF(x) : x instanceof New ? newF(x) : x instanceof Undefined ? undefinedF(x) : x instanceof Null ? nullF(x) : x instanceof HasNID ? numberF(x.number) : stringF(x.string) }
export function functionDeclarationFromExpression(x: Expression) { return x instanceof FunctionDeclaration ? x : nothing }
export function externFromExpression(x: Expression) { return x instanceof Extern ? x : nothing }
export function parameterFromExpression(x: Expression) { return x instanceof Parameter ? x : nothing }
export function arrayLiteralFromExpression(x: Expression) { return x instanceof ArrayLiteral ? x : nothing }
export function objectLiteralFromExpression(x: Expression) { return x instanceof ObjectLiteral ? x : nothing }
export function keyValueFromExpression(x: Expression) { return x instanceof KeyValue ? x : nothing }
export function binaryInlineFromExpression(x: Expression) { return x instanceof BinaryInline ? x : nothing }
export function conditionalFromExpression(x: Expression) { return x instanceof Conditional ? x : nothing }
export function functionCallFromExpression(x: Expression) { return x instanceof FunctionCall ? x : nothing }
export function arrowFunctionFromExpression(x: Expression) { return x instanceof ArrowFunction ? x : nothing }
export function newFromExpression(x: Expression) { return x instanceof New ? x : nothing }
export function undefinedFromExpression(x: Expression) { return x instanceof Undefined ? x : nothing }
export function nullFromExpression(x: Expression) { return x instanceof Null ? x : nothing }
export function numberFromExpression(x: Expression) { return numberFromID(x.id) }
export function stringFromExpression(x: Expression) { return stringFromID(x.id) }

export type JSON = JSONString | JSONNumber | JSONArray | JSONObject
export type GUIDJSON = GUIDJSONString | GUIDJSONNumber | GUIDJSONArray | GUIDJSONObject
export function jsonFromID(id: ID): Maybe<JSON> { return checkAlgebraicType<JSON>(id, [{ctor: jsonStringCtor, f: id => new JSONString(id)}, {ctor: jsonNumberCtor, f: id => new JSONNumber(id)}, {ctor: jsonArrayCtor, f: id => new JSONArray(id)}, {ctor: jsonObjectCtor, f: id => new JSONObject(id)}]) }
export function matchJSON<A>(x: JSON, jsonStringF: (x: JSONString) => A, jsonNumberF: (x: JSONNumber) => A, jsonArrayF: (x: JSONArray) => A, jsonObjectF: (x: JSONObject) => A) { return x instanceof JSONString ? jsonStringF(x) : x instanceof JSONNumber ? jsonNumberF(x) : x instanceof JSONArray ? jsonArrayF(x) : jsonObjectF(x) }
export function jsonStringFromJSON(x: JSON) { return x instanceof JSONString ? x : nothing }
export function jsonNumberFromJSON(x: JSON) { return x instanceof JSONNumber ? x : nothing }
export function jsonArrayFromJSON(x: JSON) { return x instanceof JSONArray ? x : nothing }
export function jsonObjectFromJSON(x: JSON) { return x instanceof JSONObject ? x : nothing }

export type Render = RenderCtor | RenderList | RenderNameShallow | Dispatch
export type GUIDRender = GUIDRenderCtor | GUIDRenderList | GUIDRenderNameShallow | GUIDDispatch
export function renderFromID(id: ID): Maybe<Render> { return checkAlgebraicType<Render>(id, [{ctor: renderCtorCtor, f: id => new RenderCtor(id)}, {ctor: renderListCtor, f: id => new RenderList(id)}, {ctor: renderNameShallowCtor, f: id => new RenderNameShallow(id)}, {ctor: dispatchCtor, f: id => new Dispatch(id)}]) }
export function matchRender<A>(x: Render, renderCtorF: (x: RenderCtor) => A, renderListF: (x: RenderList) => A, renderNameShallowF: (x: RenderNameShallow) => A, dispatchF: (x: Dispatch) => A) { return x instanceof RenderCtor ? renderCtorF(x) : x instanceof RenderList ? renderListF(x) : x instanceof RenderNameShallow ? renderNameShallowF(x) : dispatchF(x) }
export function renderCtorFromRender(x: Render) { return x instanceof RenderCtor ? x : nothing }
export function renderListFromRender(x: Render) { return x instanceof RenderList ? x : nothing }
export function renderNameShallowFromRender(x: Render) { return x instanceof RenderNameShallow ? x : nothing }
export function dispatchFromRender(x: Render) { return x instanceof Dispatch ? x : nothing }

export type Statement = FunctionDeclaration | Extern | Parameter | ArrayLiteral | ObjectLiteral | KeyValue | BinaryInline | Conditional | FunctionCall | ArrowFunction | New | Undefined | Null | Return | If | VariableDeclaration | HasNID | HasSID
export function statementFromID(id: ID): Maybe<Statement> { return altMaybe(checkAlgebraicType<Statement>(id, [{ctor: functionDeclarationCtor, f: id => new FunctionDeclaration(id)}, {ctor: externCtor, f: id => new Extern(id)}, {ctor: parameterCtor, f: id => new Parameter(id)}, {ctor: arrayLiteralCtor, f: id => new ArrayLiteral(id)}, {ctor: objectLiteralCtor, f: id => new ObjectLiteral(id)}, {ctor: keyValueCtor, f: id => new KeyValue(id)}, {ctor: binaryInlineCtor, f: id => new BinaryInline(id)}, {ctor: conditionalCtor, f: id => new Conditional(id)}, {ctor: functionCallCtor, f: id => new FunctionCall(id)}, {ctor: arrowFunctionCtor, f: id => new ArrowFunction(id)}, {ctor: newCtor, f: id => new New(id)}, {ctor: undefinedCtor, f: id => new Undefined(id)}, {ctor: nullCtor, f: id => new Null(id)}, {ctor: returnCtor, f: id => new Return(id)}, {ctor: ifCtor, f: id => new If(id)}, {ctor: variableDeclarationCtor, f: id => new VariableDeclaration(id)}]), () => checkNumber(id), () => checkString(id)) }
export function matchStatement<A>(x: Statement, functionDeclarationF: (x: FunctionDeclaration) => A, externF: (x: Extern) => A, parameterF: (x: Parameter) => A, arrayLiteralF: (x: ArrayLiteral) => A, objectLiteralF: (x: ObjectLiteral) => A, keyValueF: (x: KeyValue) => A, binaryInlineF: (x: BinaryInline) => A, conditionalF: (x: Conditional) => A, functionCallF: (x: FunctionCall) => A, arrowFunctionF: (x: ArrowFunction) => A, newF: (x: New) => A, undefinedF: (x: Undefined) => A, nullF: (x: Null) => A, returnF: (x: Return) => A, ifF: (x: If) => A, variableDeclarationF: (x: VariableDeclaration) => A, numberF: (x: number) => A, stringF: (x: string) => A) { return x instanceof FunctionDeclaration ? functionDeclarationF(x) : x instanceof Extern ? externF(x) : x instanceof Parameter ? parameterF(x) : x instanceof ArrayLiteral ? arrayLiteralF(x) : x instanceof ObjectLiteral ? objectLiteralF(x) : x instanceof KeyValue ? keyValueF(x) : x instanceof BinaryInline ? binaryInlineF(x) : x instanceof Conditional ? conditionalF(x) : x instanceof FunctionCall ? functionCallF(x) : x instanceof ArrowFunction ? arrowFunctionF(x) : x instanceof New ? newF(x) : x instanceof Undefined ? undefinedF(x) : x instanceof Null ? nullF(x) : x instanceof Return ? returnF(x) : x instanceof If ? ifF(x) : x instanceof VariableDeclaration ? variableDeclarationF(x) : x instanceof HasNID ? numberF(x.number) : stringF(x.string) }
export function functionDeclarationFromStatement(x: Statement) { return x instanceof FunctionDeclaration ? x : nothing }
export function externFromStatement(x: Statement) { return x instanceof Extern ? x : nothing }
export function parameterFromStatement(x: Statement) { return x instanceof Parameter ? x : nothing }
export function arrayLiteralFromStatement(x: Statement) { return x instanceof ArrayLiteral ? x : nothing }
export function objectLiteralFromStatement(x: Statement) { return x instanceof ObjectLiteral ? x : nothing }
export function keyValueFromStatement(x: Statement) { return x instanceof KeyValue ? x : nothing }
export function binaryInlineFromStatement(x: Statement) { return x instanceof BinaryInline ? x : nothing }
export function conditionalFromStatement(x: Statement) { return x instanceof Conditional ? x : nothing }
export function functionCallFromStatement(x: Statement) { return x instanceof FunctionCall ? x : nothing }
export function arrowFunctionFromStatement(x: Statement) { return x instanceof ArrowFunction ? x : nothing }
export function newFromStatement(x: Statement) { return x instanceof New ? x : nothing }
export function undefinedFromStatement(x: Statement) { return x instanceof Undefined ? x : nothing }
export function nullFromStatement(x: Statement) { return x instanceof Null ? x : nothing }
export function returnFromStatement(x: Statement) { return x instanceof Return ? x : nothing }
export function ifFromStatement(x: Statement) { return x instanceof If ? x : nothing }
export function variableDeclarationFromStatement(x: Statement) { return x instanceof VariableDeclaration ? x : nothing }
export function numberFromStatement(x: Statement) { return numberFromID(x.id) }
export function stringFromStatement(x: Statement) { return stringFromID(x.id) }

export type Type = AlgebraicType | ListType | Ctor | AtomicType
export type GUIDType = GUIDAlgebraicType | GUIDListType | GUIDCtor | GUIDAtomicType
export function typeFromID(id: ID): Maybe<Type> { return checkAlgebraicType<Type>(id, [{ctor: algebraicTypeCtor, f: id => new AlgebraicType(id)}, {ctor: listTypeCtor, f: id => new ListType(id)}, {ctor: ctorCtor, f: id => new Ctor(id)}, {ctor: atomicTypeCtor, f: id => new AtomicType(id)}]) }
export function matchType<A>(x: Type, algebraicTypeF: (x: AlgebraicType) => A, listTypeF: (x: ListType) => A, ctorF: (x: Ctor) => A, atomicTypeF: (x: AtomicType) => A) { return x instanceof AlgebraicType ? algebraicTypeF(x) : x instanceof ListType ? listTypeF(x) : x instanceof Ctor ? ctorF(x) : atomicTypeF(x) }
export function algebraicTypeFromType(x: Type) { return x instanceof AlgebraicType ? x : nothing }
export function listTypeFromType(x: Type) { return x instanceof ListType ? x : nothing }
export function ctorFromType(x: Type) { return x instanceof Ctor ? x : nothing }
export function atomicTypeFromType(x: Type) { return x instanceof AtomicType ? x : nothing }

export type WeightedEntry = NetworkEntry | HouseAdEntry
export type GUIDWeightedEntry = GUIDNetworkEntry | GUIDHouseAdEntry
export function weightedEntryFromID(id: ID): Maybe<WeightedEntry> { return checkAlgebraicType<WeightedEntry>(id, [{ctor: networkEntryCtor, f: id => new NetworkEntry(id)}, {ctor: houseAdEntryCtor, f: id => new HouseAdEntry(id)}]) }
export function matchWeightedEntry<A>(x: WeightedEntry, networkEntryF: (x: NetworkEntry) => A, houseAdEntryF: (x: HouseAdEntry) => A) { return x instanceof NetworkEntry ? networkEntryF(x) : houseAdEntryF(x) }
export function networkEntryFromWeightedEntry(x: WeightedEntry) { return x instanceof NetworkEntry ? x : nothing }
export function houseAdEntryFromWeightedEntry(x: WeightedEntry) { return x instanceof HouseAdEntry ? x : nothing }

export const
  accessKeyIdField = new GUIDField("17b303d4b1ff8f745cf761e6c9be64cb"),
  actionURLField = new GUIDField("380d186f699bf418eb78c420e1ecaefa"),
  adProbabilityField = new GUIDField("32945debf12af7f536c235bbbf18ac6d"),
  appField = new GUIDField("29bee9c65d2cec2af757bedc2fbf90aa"),
  argumentsField = new GUIDField("40c9d6a1d748f79df7704ac5629b0e76"),
  binaryOperatorField = new GUIDField("a6bd0e68fd675a8b5de2f12f48e83d07"),
  bucketField = new GUIDField("3c92672106547a6fa857e3a75f29a3e3"),
  childField = new GUIDField("b8b38542590248fddea03aa7faa9004c"),
  childrenField = new GUIDField("55944d181e596b9c1642f9687ac9fcd5"),
  closingField = new GUIDField("022d34a8b5652176e825bb0440454ab2"),
  conditionField = new GUIDField("b061582a37c4c6930f3d5c74a0201263"),
  constLetVarField = new GUIDField("da71dfbbc95473dcf8171a99caa8c117"),
  contextRenderField = new GUIDField("908e3614a53ec485765099f4f88ffd5e"),
  credentialsField = new GUIDField("9cb3cca5dcdc302d2f6f62359ba8cc38"),
  ctorField = new GUIDField("aba6ac79fd3d409da860a77c90942852"),
  ctorOrAlgebraicTypesField = new GUIDField("1088fc911436441ca7f0b569fcf72da5"),
  dField = new GUIDField("06468db341494c6d0f1a1113453b5284"),
  dataField = new GUIDField("d4220640231679254ebab0c7ab0ba283"),
  expressionField = new GUIDField("8369f4dd22a83095c4c2764c2e5502cc"),
  expressionsField = new GUIDField("6395df831571ce039f9b83d69ed2db6b"),
  extensionField = new GUIDField("614146d19462f584896cbc9c4f4b0ce8"),
  falseExpressionField = new GUIDField("b9b2efd111d42ef7e932095ad46d75bf"),
  falseStatementsField = new GUIDField("1c50936c3d59e6ef45f5f747743d8f66"),
  fetchPeriodField = new GUIDField("6d1c87782f242ff19751996c541edf7c"),
  fieldField = new GUIDField("023d9b535e0883d5f247049a587d41d6"),
  fieldsField = new GUIDField("210a5f0ea35c4677bf37192a69f0fb84"),
  forCtorField = new GUIDField("d176160129babb7724ec2656ece7d7a1"),
  functionField = new GUIDField("165790765bfebde3400846937689a88b"),
  headField = new GUIDField("a74851b7a58f4e52b72ee719b258a7b1"),
  heightField = new GUIDField("28b8f1f8e7bacfcf6b0ee28bbb8e6fa2"),
  imagesField = new GUIDField("a29ee2f11e99119d332884077a8c3e16"),
  javascriptProgramField = new GUIDField("07457861b489ce71e67206291d7b9a91"),
  jsonArrayField = new GUIDField("d8f7169ac31b569619c61ce5971f8ed4"),
  jsonNumberField = new GUIDField("c420e072625c55a4580d04e693e02a56"),
  jsonStringField = new GUIDField("bb7e08b7b66431b419ba1b2b331a6459"),
  keyField = new GUIDField("8273a691bc098cd29d21d39049e2871f"),
  keyValuePairsField = new GUIDField("76aa775e4ab2c67bda09bbe855d79cb4"),
  keyValuesField = new GUIDField("e9aca36db993952d8abb50dadb0f2833"),
  leftField = new GUIDField("8c9773b31c6b252d7a1e72e9bfde1b27"),
  lifetimeCapField = new GUIDField("e9a7f5102b4ac42d983a9fff88d7d104"),
  minimumCheckpointsPerAdField = new GUIDField("edb26438a7b13e5d77d49b3dc2bb8c1e"),
  nameField = new GUIDField("169a81aefca74e92b45e3fa03c7021df"),
  objectKeyField = new GUIDField("00936cdba9ff09ffbdaf7ade7251c493"),
  objectValueField = new GUIDField("4391d8d2309238edae2725047118f7af"),
  openingField = new GUIDField("cf1d43a4cb7bbe296f2d38c65f36cd2a"),
  parametersField = new GUIDField("6d65c89d35881c900e1418ff1db52bb9"),
  platformField = new GUIDField("e92b32a5abe83351ec2bc7085d957113"),
  renderCtorsField = new GUIDField("bc20a35da250d825fc8af115d52f879f"),
  rendersField = new GUIDField("abb3a705a157db4b6f2b0ae1a75f6439"),
  rightField = new GUIDField("12c8dcfc40feb95766d8810013c47ed5"),
  rootField = new GUIDField("8621e90c49184656ae024c94fbabd439"),
  secretAccessKeyField = new GUIDField("23e6e46577eeb97fd232639c42e866f2"),
  separatorField = new GUIDField("ce030d314931a9df1ba07c298b0cad90"),
  sha1Field = new GUIDField("b008bca2b15f47feebd5c6ee7e7afae7"),
  statementsField = new GUIDField("b00e2d33c8ef3a0188a9bab0bacea20a"),
  stringField = new GUIDField("c4c842eda0ff65ab2857163315ca35f5"),
  tailField = new GUIDField("e53f14ab72eb40f590e5ae53fb53e988"),
  tiersField = new GUIDField("a98114eed3d688c868a150465530d989"),
  timeIntervalPerAdField = new GUIDField("bb230ff584c4d4da4133e31d7c733e41"),
  transformationsField = new GUIDField("54bf493c5cefe12df08037456ffc2f7a"),
  trueExpressionField = new GUIDField("689487ddbc71bf382517f6cfa4d6dcc4"),
  trueStatementsField = new GUIDField("e1b260faeec77c426545aa4a541a0f18"),
  typeField = new GUIDField("223a9b55b8a1413497879a52e5dea939"),
  urlField = new GUIDField("6b251d45b58e759904a6dc7caab743a8"),
  valueField = new GUIDField("a417f1b7167cee31537cf561bc7396ad"),
  viewsField = new GUIDField("8d27c204f7294593b2f9f3c3af4a477d"),
  weightField = new GUIDField("7bce82bcee93437a748dcd693caa2cc0"),
  widthField = new GUIDField("7e36f32e37b80f495dbba6bb75ec2ec5"),
  algebraicTypeCtor = new GUIDCtor("ba181d67665d4e57b9fa1694dbdacbca"),
  andCtor = new GUIDCtor("89da9a377b0116af9531275a81545c32"),
  appCtor = new GUIDCtor("2a19592fa5047f6ad1ef58d885611c6f"),
  appPlatformCtor = new GUIDCtor("ada9d971322ed8c692b2946097993f53"),
  arrayLiteralCtor = new GUIDCtor("45608f8cc49f5c9e3cf5e37dcc32e842"),
  arrowFunctionCtor = new GUIDCtor("1bbe86935fc983f3107a64211436e63b"),
  assignmentCtor = new GUIDCtor("9df5e800c0bcb78c0cd46def7a8350b3"),
  atomicTypeCtor = new GUIDCtor("4e63cb391b72641490acd1b3e2619ddb"),
  awsCredentialsCtor = new GUIDCtor("bbeaf300c9c0e027dac6d7c91c3fab66"),
  binaryInlineCtor = new GUIDCtor("42d7915ca3b0acc8921ea2135bef3719"),
  blockCtor = new GUIDCtor("69578eb3ad4ec4d286443c21cf1d78fc"),
  bradParamsCtor = new GUIDCtor("4f4e5a4bb6f56d5c618156e9d7a539d5"),
  conditionalCtor = new GUIDCtor("8f16557541a11785df1072f59a7e0395"),
  constCtor = new GUIDCtor("7b52c4f1319c085ae838e07e2cf88c85"),
  ctorCtor = new GUIDCtor("e35d27082ac44a759a4e4c0535f243d7"),
  ctorFieldCtor = new GUIDCtor("68f4a71d7c13980c249e939358fc4f17"),
  descendCtor = new GUIDCtor("7a6f518f8b877cf182c427e98e65a5b5"),
  differenceCtor = new GUIDCtor("8d2f13893c49fb07f159439c663263c4"),
  dispatchCtor = new GUIDCtor("db98a62666a14eb8d942ec1b82ffdb70"),
  dotCtor = new GUIDCtor("d6e2fe096d28de66598ba3a98dee2cb3"),
  emptyListCtor = new GUIDCtor("51fb7a7a95d4486bb197509fd53dec2d"),
  equalsCtor = new GUIDCtor("afd0cd8ae646f670f738f3a5a525f650"),
  evaluateCtor = new GUIDCtor("dddff915232a08ab20faef01a049f78a"),
  externCtor = new GUIDCtor("0d83cb4ebb1a4c901952091fd30ee808"),
  fieldCtor = new GUIDCtor("a963494fb49742f4a0a2b12011ac3cbe"),
  functionCallCtor = new GUIDCtor("214441b0d718a846fd4b19a5f76c7996"),
  functionDeclarationCtor = new GUIDCtor("69822a9fdb0e16d4a833c20e8e2b6a22"),
  greaterThanCtor = new GUIDCtor("50590db86fc8a3d40f7fbbc3db6810dd"),
  greaterThanOrEqualToCtor = new GUIDCtor("bac3b11f719107d667412ecaa6d12446"),
  houseAdEntryCtor = new GUIDCtor("b33d5e998f7bfbdcce857e09582edda5"),
  houseAdImageCtor = new GUIDCtor("e912c5ddb1cad6518f36ab50e240fba7"),
  ifCtor = new GUIDCtor("cd38287e9184f0062b0f2cbd3f50da7a"),
  javascriptProgramCtor = new GUIDCtor("4ab3f0dc1c14bd4bfffe937802930d05"),
  jsonArrayCtor = new GUIDCtor("e08c7a6c29586e9ac1e276b48fa93f35"),
  jsonNumberCtor = new GUIDCtor("4d2089e72be2dda1d8ad71b990b4ddcd"),
  jsonObjectCtor = new GUIDCtor("8d6d0983c3b1304c72012e63f274858c"),
  jsonStringCtor = new GUIDCtor("fe84cffd6372374ef70f6fcc195484cc"),
  keyValueCtor = new GUIDCtor("029acef6faba9679751166678a4784db"),
  keyValuePairCtor = new GUIDCtor("c3f2bbad56fca1f4e190142cb677e4d5"),
  labelCtor = new GUIDCtor("50b7b81fb70b4b32a57953d88563a3e0"),
  lessThanCtor = new GUIDCtor("2860c0d881ec0a93c8f5acfcb633598c"),
  lessThanOrEqualToCtor = new GUIDCtor("54e2b2d9f4ed7ff3cd0cf587916f378f"),
  letCtor = new GUIDCtor("5556f62d14253cb7b58887ad5f5da099"),
  lineCtor = new GUIDCtor("732ed87bdb114213ddaf17ce6b167d9c"),
  listTypeCtor = new GUIDCtor("6410d2232b824a38bf61780cc1a12886"),
  loadAWSCtor = new GUIDCtor("3184682ef8bff8421abc525d3c292e09"),
  loadJSONCtor = new GUIDCtor("a33b56ef93a72c0ae3ab1709cf308928"),
  moduleCtor = new GUIDCtor("3c0e5c714e551ef48390f803fa17569b"),
  networkEntryCtor = new GUIDCtor("191b15b7460b928c1dc29d518f16eb18"),
  newCtor = new GUIDCtor("910127d3651dc8dc1985d624abb8881e"),
  nonemptyListCtor = new GUIDCtor("f0408beb29c74dc7bc20dc461104e949"),
  notEqualsCtor = new GUIDCtor("6cbf47ded0621a33c4d712d7bcc0a2a3"),
  nullCtor = new GUIDCtor("3cea950fc835841af2afc518a8dfe688"),
  objectLiteralCtor = new GUIDCtor("752411002f2f8630440d4d75ce00a7fd"),
  orCtor = new GUIDCtor("b25858d75fd4a4bc97352c13367760d7"),
  parameterCtor = new GUIDCtor("ee0ccdd47b67323bb4442be189813268"),
  platformCtor = new GUIDCtor("31c3b378fad19ce374296bc7b93a6c32"),
  productCtor = new GUIDCtor("64663d851ac8fd5667c29f9eb836a16f"),
  putAWSCtor = new GUIDCtor("a44c85987be19b3ea473ae0be2e048cb"),
  putAWSSucceededCtor = new GUIDCtor("fe3f998b2fd0a3700a9d29a9460821d1"),
  quotientCtor = new GUIDCtor("d4dfa5bba3f44b1c8db13b38c8504939"),
  renderCtorCtor = new GUIDCtor("e4ba7b3350b6ba78485c3b0fe66d74e7"),
  renderListCtor = new GUIDCtor("bfe62ce7b212eb753823b3a5f244c404"),
  renderNameShallowCtor = new GUIDCtor("bcb942240e9f0e111b0cf985360c5188"),
  returnCtor = new GUIDCtor("bba85fdde889e435725b437d82d41596"),
  rootViewsCtor = new GUIDCtor("9949c2dded2a41aaaeb349b968975fde"),
  strictEqualsCtor = new GUIDCtor("9e7e02107fb914004fd34a5867652c5c"),
  strictNotEqualsCtor = new GUIDCtor("c57de19bbff6519003d2959e8f3d0123"),
  sumCtor = new GUIDCtor("66a2a6cacfd8f5bcddb200964e936457"),
  undefinedCtor = new GUIDCtor("61bb6c05ed466f923d8d6ee0b0c1295b"),
  varCtor = new GUIDCtor("9eb7b6c08dea5bd3118ab4e52a67af55"),
  variableDeclarationCtor = new GUIDCtor("b98dd2eb21483f6acd304be44ef40c9c"),
  binaryOperatorAlgebraicType = new GUIDAlgebraicType("e27fa156592ee2f9b738bbfe9d66dff6"),
  constLetVarAlgebraicType = new GUIDAlgebraicType("556afcdc910c5bbaa3ecb24014389a0a"),
  ctorOrAlgebraicTypeAlgebraicType = new GUIDAlgebraicType("63a2e588102f5e5453316d46fea2f02b"),
  dAlgebraicType = new GUIDAlgebraicType("583bb47238632e875ffea7544a909e1a"),
  expressionAlgebraicType = new GUIDAlgebraicType("32134c040cd504435205a551d2d4fb66"),
  jsonAlgebraicType = new GUIDAlgebraicType("52120b6d534a2fffe968f0c055df6ca8"),
  listAlgebraicType = new GUIDAlgebraicType("e06b24ad99bf4e14a368aaf93bfb143b"),
  renderAlgebraicType = new GUIDAlgebraicType("86a592afa6af938c63acde8fcc753aba"),
  rootViewsAlgebraicType = new GUIDAlgebraicType("bb04aa07996b4db5b9ddfae2287c2901"),
  statementAlgebraicType = new GUIDAlgebraicType("1f0ef6c04ffe3cddcb354d92492e0bbd"),
  typeAlgebraicType = new GUIDAlgebraicType("17458686b71245d092a8c930140c32c5"),
  weightedEntryAlgebraicType = new GUIDAlgebraicType("9ce59e032d7570cd200f1b30589d0c15"),
  numberAtomicType = new GUIDAtomicType("f97bcfc1c3a84a45958307a512f05954"),
  stringAtomicType = new GUIDAtomicType("70d1d53107174f88858da0cdad6050d5")