import { minidenticon } from 'minidenticons'

type Props = {
  value: string
  size?: number
}

export function Identicon(props: Props) {
  const size = props.size ?? 16
  const svg = minidenticon(props.value)
  return <img src={`data:image/svg+xml;utf8,${encodeURIComponent(svg)}`} width={size} height={size} />
}
