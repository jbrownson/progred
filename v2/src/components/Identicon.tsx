import { minidenticon } from 'minidenticons'

type Props = {
  value: string
  size?: number
  label?: boolean
}

export function Identicon(props: Props) {
  const size = props.size ?? 16
  const svg = minidenticon(props.value)
  return (
    <img
      src={`data:image/svg+xml;utf8,${encodeURIComponent(svg)}`}
      width={size}
      height={size}
      class={props.label ? "identicon label" : "identicon"}
    />
  )
}
