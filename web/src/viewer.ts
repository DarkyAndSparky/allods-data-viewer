import {
  AmbientLight,
  BufferAttribute,
  BufferGeometry,
  Color,
  DirectionalLight,
  Group,
  Mesh as ThreeMesh,
  MeshStandardMaterial,
  PerspectiveCamera,
  Scene,
  WebGLRenderer,
} from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { Mesh } from "./mesh";

/** A small orbit viewer for one untextured model. Call `dispose` when it is no longer shown. */
export class ModelViewer {
  private renderer: WebGLRenderer;
  private scene = new Scene();
  private camera = new PerspectiveCamera(45, 1, 0.01, 10000);
  private controls: OrbitControls;
  private material = new MeshStandardMaterial({ color: 0xc9b98f, roughness: 0.8, metalness: 0.05, flatShading: false });
  private geometry = new BufferGeometry();
  private mesh: ThreeMesh;
  private pivot = new Group();
  private observer: ResizeObserver;
  private frame = 0;
  private radius = 1;
  private center: [number, number, number] = [0, 0, 0];

  /** Throws when WebGL is not available. */
  constructor(private host: HTMLElement, data: Mesh) {
    this.renderer = new WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.scene.background = new Color(0x16181d);
    this.host.append(this.renderer.domElement);

    this.geometry.setAttribute("position", new BufferAttribute(data.positions, 3));
    this.geometry.setAttribute("uv", new BufferAttribute(data.uvs, 2));
    this.geometry.setIndex(new BufferAttribute(data.indices, 1));
    this.geometry.computeVertexNormals();
    this.material.side = 2; // double sided: winding is not verified yet
    this.mesh = new ThreeMesh(this.geometry, this.material);
    this.pivot.add(this.mesh);
    this.scene.add(this.pivot);

    this.scene.add(new AmbientLight(0xffffff, 0.55));
    const sun = new DirectionalLight(0xffffff, 2.2);
    sun.position.set(1, 2, 3);
    this.camera.add(sun);
    this.scene.add(this.camera);

    const [x0, y0, z0, x1, y1, z1] = data.bounds;
    this.center = [(x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2];
    this.radius = Math.max(Math.hypot(x1 - x0, y1 - y0, z1 - z0) / 2, 1e-3);
    this.mesh.position.set(-this.center[0], -this.center[1], -this.center[2]);

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.resetView();

    this.observer = new ResizeObserver(() => this.resize());
    this.observer.observe(this.host);
    this.resize();
    const tick = () => {
      this.frame = requestAnimationFrame(tick);
      this.controls.update();
      this.renderer.render(this.scene, this.camera);
    };
    tick();
  }

  /** Puts the camera in front of the model, looking at its centre. */
  resetView(): void {
    this.camera.near = this.radius / 100;
    this.camera.far = this.radius * 100;
    this.camera.position.set(0, this.radius * 0.6, this.radius * 2.6);
    this.camera.updateProjectionMatrix();
    this.controls.target.set(0, 0, 0);
    this.controls.update();
  }

  setWireframe(on: boolean): void {
    this.material.wireframe = on;
  }

  /** Many game formats are Z-up; turning the model by -90° about X makes it stand. */
  setZUp(on: boolean): void {
    this.pivot.rotation.x = on ? -Math.PI / 2 : 0;
  }

  private resize(): void {
    const w = Math.max(this.host.clientWidth, 1);
    const h = Math.max(this.host.clientHeight, 1);
    this.renderer.setSize(w, h, false);
    this.renderer.domElement.style.width = "100%";
    this.renderer.domElement.style.height = "100%";
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }

  dispose(): void {
    cancelAnimationFrame(this.frame);
    this.observer.disconnect();
    this.controls.dispose();
    this.geometry.dispose();
    this.material.dispose();
    this.renderer.dispose();
    this.renderer.domElement.remove();
  }
}
