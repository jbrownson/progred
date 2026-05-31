import * as React from "react"
import * as THREE from "three"
import { OrbitControls } from "three/addons/controls/OrbitControls.js"
import { maybe, maybeMap, bindMaybe, fromMaybe, mapMaybe, Maybe } from "../../lib/Maybe"
import { Implicit, ImplicitSolid, matchImplicit, matchScene3DObject, Polyline3D, Scene3D, Vec3 } from "../graph"
import { withEnvironment } from "../Environment"
import { useDContext } from "../render/DContext"
import { FidgetImplicit, FidgetMesh, fidgetMeshFromImplicit } from "../../fidget/FidgetMesh"

type SceneObject =
  | {type: "polyline", points: THREE.Vector3[], color: Maybe<string>}
  | {type: "implicit", mesh: FidgetMesh}

function bounds(objects: SceneObject[]): {center: THREE.Vector3, radius: number, floorY: number} {
  const box = new THREE.Box3()
  objects.forEach(object => {
    if (object.type === "polyline") object.points.forEach(point => box.expandByPoint(point))
    else fidgetMeshPoints(object.mesh).forEach(point => box.expandByPoint(point)) })
  if (box.isEmpty()) return {center: new THREE.Vector3(), radius: 1, floorY: 0}
  const sphere = new THREE.Sphere()
  box.getBoundingSphere(sphere)
  return {center: sphere.center, radius: Math.max(sphere.radius, 0.1), floorY: box.min.y} }

function colorFromString(color: string | undefined): THREE.ColorRepresentation {
  return color || "#3f76d3" }

function vector3(vec3: Vec3): Maybe<THREE.Vector3> {
  return bindMaybe(vec3.x, x =>
    bindMaybe(vec3.y, y =>
      mapMaybe(vec3.z, z => new THREE.Vector3(x, y, z)))) }

function points(polyline: Polyline3D) {
  return maybe(polyline.points, () => [], vec3s => maybeMap(vec3s, vector3)) }

function sceneObjects(scene: Scene3D): Promise<SceneObject[]> {
  return Promise.all(maybe(scene.objects, () => [], objects => objects.map(sceneObject)))
    .then(objects => objects.filter(object => object !== undefined)) }

function sceneObject(object: Polyline3D | ImplicitSolid): Promise<Maybe<SceneObject>> {
  return matchScene3DObject<Promise<Maybe<SceneObject>>>(
    object,
    async polyline => ({type: "polyline", points: points(polyline), color: polyline.color}),
    async solid => {
      const implicit = bindMaybe(solid.implicit, implicitJSON)
      return implicit === undefined ? undefined : {type: "implicit", mesh: await fidgetMeshFromImplicit(implicit, fromMaybe(solid.depth, () => 5), fromMaybe(solid.scale, () => 2))} }) }

function implicitJSON(implicit: Implicit): Maybe<FidgetImplicit> {
  return matchImplicit<Maybe<FidgetImplicit>>(
    implicit,
    () => ({op: "x"}),
    () => ({op: "y"}),
    () => ({op: "z"}),
    constant => mapMaybe(constant.constant, value => ({op: "constant", value})),
    add => binaryImplicitJSON("add", add.a, add.b),
    subtract => binaryImplicitJSON("subtract", subtract.a, subtract.b),
    multiply => binaryImplicitJSON("multiply", multiply.a, multiply.b),
    divide => binaryImplicitJSON("divide", divide.a, divide.b),
    minimum => binaryImplicitJSON("minimum", minimum.a, minimum.b),
    maximum => binaryImplicitJSON("maximum", maximum.a, maximum.b)) }

function binaryImplicitJSON(op: "add" | "subtract" | "multiply" | "divide" | "minimum" | "maximum", a: Maybe<Implicit>, b: Maybe<Implicit>): Maybe<FidgetImplicit> {
  return bindMaybe(a, a =>
    bindMaybe(b, b =>
      bindMaybe(implicitJSON(a), a =>
        mapMaybe(implicitJSON(b), b => ({op, a, b}))))) }

function addSceneObjects(threeScene: THREE.Scene, objects: SceneObject[]) {
  objects.forEach(object => {
    if (object.type === "polyline") {
      const geometry = new THREE.BufferGeometry().setFromPoints(object.points)
      const material = new THREE.LineBasicMaterial({color: colorFromString(object.color)})
      threeScene.add(new THREE.Line(geometry, material))
    } else {
      const geometry = fidgetMeshGeometry(object.mesh)
      const material = new THREE.MeshStandardMaterial({color: "#7fb98d", roughness: 0.72, metalness: 0, flatShading: true})
      threeScene.add(new THREE.Mesh(geometry, material)) }}) }

function addSceneLights(threeScene: THREE.Scene) {
  threeScene.add(new THREE.HemisphereLight("#f6fbff", "#7d858d", 0.75))
  const key = new THREE.DirectionalLight("#ffffff", 2.8)
  key.position.set(3, 5, 4)
  threeScene.add(key)
  const fill = new THREE.DirectionalLight("#c7dcff", 0.8)
  fill.position.set(-4, 2, -3)
  threeScene.add(fill)
  const rim = new THREE.DirectionalLight("#ffffff", 1.1)
  rim.position.set(-2, 3, 5)
  threeScene.add(rim)
}

function fidgetMeshPoints(mesh: FidgetMesh) {
  let points: THREE.Vector3[] = []
  for (let i = 0; i < mesh.positions.length; i += 3)
    points.push(new THREE.Vector3(mesh.positions[i], mesh.positions[i + 1], mesh.positions[i + 2]))
  return points }

function fidgetMeshGeometry(mesh: FidgetMesh) {
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(mesh.positions, 3))
  geometry.setIndex(mesh.indices)
  return geometry }

export function Scene3DComponent(props: {scene: Scene3D}) {
  const context = useDContext()
  const containerRef = React.useRef<HTMLDivElement | null>(null)
  React.useEffect(() => {
    const container = containerRef.current
    if (!container) return
    let disposed = false
    const threeScene = new THREE.Scene()
    threeScene.background = new THREE.Color("#fbfbfa")
    const renderer = new THREE.WebGLRenderer({antialias: true})
    renderer.setPixelRatio(window.devicePixelRatio)
    container.appendChild(renderer.domElement)

    const camera = new THREE.PerspectiveCamera(45, 1, 0.01, 1000)
    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enableDamping = true

    addSceneLights(threeScene)

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

    const animate = () => {
      if (disposed) return
      controls.update()
      renderer.render(threeScene, camera)
      requestAnimationFrame(animate) }
    animate()

    withEnvironment(context.environment, () => sceneObjects(props.scene)).then(objects => {
      if (disposed) return
      const {center, radius, floorY} = bounds(objects)
      camera.position.copy(center.clone().add(new THREE.Vector3(radius * 2.2, radius * 1.7, radius * 2.2)))
      camera.near = Math.max(radius / 1000, 0.001)
      camera.far = Math.max(radius * 100, 100)
      camera.updateProjectionMatrix()
      controls.target.copy(center)
      const grid = new THREE.GridHelper(radius * 4, 12, "#d0d3d8", "#ebecef")
      grid.position.copy(center)
      grid.position.y = floorY
      threeScene.add(grid)
      addSceneObjects(threeScene, objects) }).catch(error => {
        if (!disposed) console.error(error) })

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
