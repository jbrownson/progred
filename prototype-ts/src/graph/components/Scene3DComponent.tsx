import * as React from "react"
import * as THREE from "three"
import { OrbitControls } from "three/addons/controls/OrbitControls.js"
import { maybe, maybeMap, bindMaybe, mapMaybe, Maybe } from "../../lib/Maybe"
import { matchScene3DObject, Polyline3D, Scene3D, Vec3 } from "../graph"
import { withEnvironment } from "../Environment"
import { useDContext } from "../render/DContext"

function bounds(scene: Scene3D): {center: THREE.Vector3, radius: number} {
  const box = new THREE.Box3()
  polylines(scene).forEach(polyline => points(polyline).forEach(point => box.expandByPoint(point)))
  if (box.isEmpty()) return {center: new THREE.Vector3(), radius: 1}
  const sphere = new THREE.Sphere()
  box.getBoundingSphere(sphere)
  return {center: sphere.center, radius: Math.max(sphere.radius, 0.1)} }

function colorFromString(color: string | undefined): THREE.ColorRepresentation {
  return color || "#3f76d3" }

function vector3(vec3: Vec3): Maybe<THREE.Vector3> {
  return bindMaybe(vec3.x, x =>
    bindMaybe(vec3.y, y =>
      mapMaybe(vec3.z, z => new THREE.Vector3(x, y, z)))) }

function points(polyline: Polyline3D) {
  return maybe(polyline.points, () => [], vec3s => maybeMap(vec3s, vector3)) }

function polylines(scene: Scene3D) {
  return maybe(scene.objects, () => [], objects => objects.map(object =>
    matchScene3DObject(object, polyline => polyline))) }

function addSceneObjects(threeScene: THREE.Scene, scene: Scene3D) {
  polylines(scene).forEach(polyline => {
    const geometry = new THREE.BufferGeometry().setFromPoints(points(polyline))
    const material = new THREE.LineBasicMaterial({color: colorFromString(polyline.color)})
    threeScene.add(new THREE.Line(geometry, material)) }) }

export function Scene3DComponent(props: {scene: Scene3D}) {
  const context = useDContext()
  const containerRef = React.useRef<HTMLDivElement | null>(null)
  React.useEffect(() => {
    const container = containerRef.current
    if (!container) return
    const threeScene = new THREE.Scene()
    threeScene.background = new THREE.Color("#fbfbfa")
    const renderer = new THREE.WebGLRenderer({antialias: true})
    renderer.setPixelRatio(window.devicePixelRatio)
    container.appendChild(renderer.domElement)

    const camera = new THREE.PerspectiveCamera(45, 1, 0.01, 1000)
    const {center, radius} = withEnvironment(context.environment, () => bounds(props.scene))
    camera.position.copy(center.clone().add(new THREE.Vector3(radius * 2.2, radius * 1.7, radius * 2.2)))
    camera.near = Math.max(radius / 1000, 0.001)
    camera.far = Math.max(radius * 100, 100)
    camera.updateProjectionMatrix()

    const controls = new OrbitControls(camera, renderer.domElement)
    controls.target.copy(center)
    controls.enableDamping = true

    threeScene.add(new THREE.HemisphereLight("#ffffff", "#d8d8d8", 2.0))
    const grid = new THREE.GridHelper(radius * 4, 12, "#d0d3d8", "#ebecef")
    grid.position.copy(center)
    threeScene.add(grid)
    withEnvironment(context.environment, () => addSceneObjects(threeScene, props.scene))

    const resize = () => {
      const rect = container.getBoundingClientRect()
      const width = Math.max(1, rect.width)
      const height = Math.max(1, rect.height)
      renderer.setSize(width, height, false)
      camera.aspect = width / height
      camera.updateProjectionMatrix() }
    const resizeObserver = new ResizeObserver(resize)
    resizeObserver.observe(container)
    resize()

    let disposed = false
    const animate = () => {
      if (disposed) return
      controls.update()
      renderer.render(threeScene, camera)
      requestAnimationFrame(animate) }
    animate()

    return () => {
      disposed = true
      resizeObserver.disconnect()
      controls.dispose()
      threeScene.traverse(object => {
        const mesh = object as THREE.Object3D & {geometry?: THREE.BufferGeometry, material?: THREE.Material}
        mesh.geometry?.dispose()
        mesh.material?.dispose() })
      renderer.dispose()
      renderer.domElement.remove() } }, [context.environment, props.scene])
  return <div ref={containerRef} className="scene3DView" />
}
