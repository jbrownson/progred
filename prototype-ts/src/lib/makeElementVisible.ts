export function makeElementVisible(element: HTMLElement, scrollParent: HTMLElement) {
  if (scrollParent.scrollTop > element.offsetTop) { scrollParent.scrollTop = element.offsetTop }
  else if (scrollParent.scrollTop + scrollParent.clientHeight < element.offsetTop + element.offsetHeight) {
    scrollParent.scrollTop = element.offsetTop + element.offsetHeight - scrollParent.clientHeight }}
