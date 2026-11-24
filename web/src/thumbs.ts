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
import { parseMesh } from "./mesh";

/** Side of a model thumbnail in pixels. */
export const THUMB_SIZE = 160;
/** Models with more indices than this get no thumbnail (they would stall the window). */
const MAX_INDICES = 3_000_000;

/** Draws small still pictures of models with one shared offscreen renderer. */
export class ModelSnapshot {
  private renderer: WebGLRenderer | null = null;
  private scene = new Scene();
  private camera = new PerspectiveCamera(40, 1, 0.01, 10000);
  private material = new MeshStandardMaterial({ color: 0xc9b98f, roughness: 0.8, metalness: 0.05, side: 2 });

  constructor() {
    this.scene.background = new Color(0x16181d);
    this.scene.add(new AmbientLight(0xffffff, 0.55));
    const sun = new DirectionalLight(0xffffff, 2.2);
    sun.position.set(1, 2, 3);
    this.camera.add(sun);
    this.scene.add(this.camera);
  }

  /** PNG of the model in front view, or `null` when WebGL or the data is unusable. */
  async render(buf: ArrayBuffer): Promise<Blob | null> {
    let mesh;
    try {
      mesh = parseMesh(buf);
    } catch {
      return null;
    }
    if (mesh.indices.length === 0 || mesh.indices.length > MAX_INDICES) return null;
    if (!this.renderer) {
      try {
        this.renderer = new WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
        this.renderer.setPixelRatio(1);
        this.renderer.setSize(THUMB_SIZE, THUMB_SIZE, false);
      } catch {
        return null;
      }
    }
    const geometry = new BufferGeometry();
    geometry.setAttribute("position", new BufferAttribute(mesh.positions, 3));
    geometry.setIndex(new BufferAttribute(mesh.indices, 1));
    geometry.computeVertexNormals();
    const pivot = new Group();
    const body = new ThreeMesh(geometry, this.material);
    const [x0, y0, z0, x1, y1, z1] = mesh.bounds;
    body.position.set(-(x0 + x1) / 2, -(y0 + y1) / 2, -(z0 + z1) / 2);
    pivot.add(body);
    this.scene.add(pivot);
    const radius = Math.max(Math.hypot(x1 - x0, y1 - y0, z1 - z0) / 2, 1e-3);
    const distance = (radius / Math.sin((this.camera.fov * Math.PI) / 360)) * 1.1;
    this.camera.near = radius / 100;
    this.camera.far = radius * 100 + distance;
    this.camera.position.set(0, distance * 0.25, distance * 0.97);
    this.camera.lookAt(0, 0, 0);
    this.camera.updateProjectionMatrix();
    this.renderer.render(this.scene, this.camera);
    const canvas = this.renderer.domElement;
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
    this.scene.remove(pivot);
    geometry.dispose();
    return blob;
  }

  dispose(): void {
    this.renderer?.dispose();
    this.renderer = null;
    this.material.dispose();
  }
}

/** Runs at most `limit` jobs at a time, in the order they were added. */
export class JobQueue {
  private running = 0;
  private waiting: (() => void)[] = [];
  constructor(private limit: number) {}

  add<T>(job: () => Promise<T>): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      const start = (): void => {
        this.running++;
        job()
          .then(resolve, reject)
          .finally(() => {
            this.running--;
            this.waiting.shift()?.();
          });
      };
      if (this.running < this.limit) start();
      else this.waiting.push(start);
    });
  }

  /** Drops the jobs that have not started yet. */
  clear(): void {
    this.waiting = [];
  }
}
