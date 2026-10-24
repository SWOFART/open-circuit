import { Application, Graphics } from "pixi.js";

export class EditorCore {
  private readonly application = new Application();
  private destroyed = false;
  private initialized = false;

  async mount(container: HTMLElement): Promise<void> {
    try {
      await this.application.init({
        antialias: true,
        autoDensity: true,
        background: "#111827",
        resolution: window.devicePixelRatio || 1,
        resizeTo: container,
      });
    } catch (error) {
      if (!this.destroyed) {
        throw error;
      }
      return;
    }

    if (this.destroyed) {
      this.application.destroy({ removeView: true });
      return;
    }

    this.initialized = true;
    container.appendChild(this.application.canvas);
    this.drawStarterGrid();
  }

  destroy(): void {
    this.destroyed = true;

    if (this.initialized) {
      this.application.destroy({ removeView: true });
      this.initialized = false;
    }
  }

  private drawStarterGrid(): void {
    const grid = new Graphics();
    const spacing = 24;

    for (let x = 0; x <= this.application.screen.width; x += spacing) {
      grid.moveTo(x, 0).lineTo(x, this.application.screen.height);
    }

    for (let y = 0; y <= this.application.screen.height; y += spacing) {
      grid.moveTo(0, y).lineTo(this.application.screen.width, y);
    }

    grid.stroke({ color: 0x263244, width: 1, alpha: 0.65 });
    this.application.stage.addChild(grid);
  }
}
