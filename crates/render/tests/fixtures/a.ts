import { render } from "./view";
export class Session<T> extends Base {
  private readonly id: string = `s-${Date.now()}`;
  async load(path: string): Promise<boolean> { return await fetch(path).then(r => r.ok) && true; }
}
