import { biquad, response } from './eq-curve';

describe('the EQ curve', () => {
  it('is as the engine hears the bands', () => {
    const peak = biquad('peak', 1000, 6, 1, 48000);
    expect(response(peak, 1000, 48000)).toBeCloseTo(6, 1);
    expect(Math.abs(response(peak, 50, 48000))).toBeLessThan(0.1);
    const low = biquad('lowShelf', 100, -12, 0.707, 48000);
    expect(response(low, 20, 48000)).toBeCloseTo(-12, 0);
    expect(Math.abs(response(low, 5000, 48000))).toBeLessThan(0.1);
  });
});
