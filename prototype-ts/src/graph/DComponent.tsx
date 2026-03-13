import * as React from 'react'
import { concatMap, intersperse, join } from "../lib/Array"
import { mapMaybe, maybeMap, nothing } from "../lib/Maybe"
import { cursorFromD } from "./cursorFromD"
import { D, matchD } from "./D"
import { environment } from "./Environment"
import { NumberEditorComponent } from "./NumberEditorComponent"
import { PlaceholderComponent } from "./PlaceholderComponent"
import { SelectionState } from "./selectionIfSelected"
import { StringEditorComponent } from "./StringEditorComponent"

export class DComponent extends React.Component<{d: D, depth: number, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void}, {}> {
  children: (DComponent | PlaceholderComponent | StringEditorComponent | NumberEditorComponent)[]
  onScroll() { this.children.forEach(child => child.onScroll()) }
  render(): JSX.Element {
    this.children = []
    let addChild = (child: DComponent | PlaceholderComponent | StringEditorComponent | NumberEditorComponent | null) => { if (child) this.children.push(child) }
    return matchD(this.props.d,
      block => <span>{concatMap(block.children, (d, index) => [
        <br key={`br${index}`} />,
        <span key={"span" + index} style={{width: 13*(this.props.depth+1)+"px", display: "inline-block"}} />,
        <DComponent ref={addChild} d={d} depth={this.props.depth + 1} scrollParent={this.props.scrollParent} runE={this.props.runE} />])}</span>,
      line => <span>{line.children.map((d, index) => <DComponent ref={addChild} key={index} d={d} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} />)}</span>,
      dText => <span onClick={e => { e.stopPropagation(); this.props.runE(() => mapMaybe(cursorFromD(this.props.d), cursor => environment().selection = ({cursor}))) }}>{dText.string}</span>,
      dList => dList.children.length <= 1
        // TOOD probably something to factor out of these two clauses
        ? <span><span onClick={e => { e.stopPropagation(); this.props.runE(() => mapMaybe(cursorFromD(this.props.d), cursor => environment().selection = ({cursor}))) }}>{dList.opening}</span><span onClick={e => { e.stopPropagation(); this.props.runE(() => dList.clickBefore(0)) }}> </span>{
          dList.children.map(child => <DComponent ref={addChild} d={child} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} />) }
          <span onClick={e => { e.stopPropagation(); this.props.runE(() => dList.clickBefore(dList.children.length)) }}> {dList.closing}</span></span>
        : <span><span onClick={e => { e.stopPropagation(); this.props.runE(() => mapMaybe(cursorFromD(this.props.d), cursor => environment().selection = ({cursor}))) }}>{dList.opening}</span><span onClick={e => { e.stopPropagation(); this.props.runE(() => dList.clickBefore(0)) }}> </span>{join(intersperse(
          dList.children.map(child => [<br />, <span style={{width: 13*(this.props.depth+1)+"px", display: "inline-block"}} />, <DComponent ref={addChild} d={child} depth={this.props.depth + 1} scrollParent={this.props.scrollParent} runE={this.props.runE} />]),
          i => [<span onClick={e => { e.stopPropagation(); this.props.runE(() => dList.clickBefore(i)) }}>{dList.separator}</span>]))}
          <span onClick={e => { e.stopPropagation(); this.props.runE(() => dList.clickBefore(dList.children.length)) }}> {dList.closing}</span></span>,
      descend => {
        let classNames = maybeMap([[descend.selectionState === SelectionState.Selected, "selected"], [descend.unmatching, "unmatching"], [descend.selectionState === SelectionState.Hinted, "hinted"]] as [boolean, string][], ([boolean, className]) => boolean ? className : nothing)
        return <span className={classNames.join(" ")}><DComponent ref={addChild} d={descend.child} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} /></span> },
      label => <DComponent ref={addChild} d={label.child} depth={this.props.depth} scrollParent={this.props.scrollParent} runE={this.props.runE} />,
      button => <input type="button" value={button.text} onClick={e => { e.stopPropagation(); this.props.runE(button.action) }} />,
      placeholder => <PlaceholderComponent ref={addChild} placeholder={placeholder} scrollParent={this.props.scrollParent} runE={this.props.runE} />,
      stringEditor => <StringEditorComponent ref={addChild} stringEditor={stringEditor} runE={this.props.runE} />,
      numberEditor => <NumberEditorComponent ref={addChild} numberEditor={numberEditor} runE={this.props.runE} /> )}}