/** @anchor ui:journal:SkeletonBar
 * @tags ui */

export function SkeletonBar(props: { width?: string; height?: string; class?: string }) {
  return (
    <div
      class={`bg-container-border/15 skeleton-shimmer ${props.class ?? ''}`}
      style={{ width: props.width ?? '100%', height: props.height ?? '12px' }}
    />
  )
}
