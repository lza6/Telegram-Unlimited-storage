import { describe, it, expect } from 'vitest';
import { formatTime, formatSize } from './utils';

describe('formatTime', () => {
  it('formats 0 seconds', () => {
    expect(formatTime(0)).toBe('0秒');
  });

  it('formats seconds only', () => {
    expect(formatTime(45)).toBe('45秒');
  });

  it('formats 60 seconds as 1分', () => {
    expect(formatTime(60)).toBe('1分');
  });

  it('formats minutes and seconds', () => {
    expect(formatTime(65)).toBe('1分5秒');
  });

  it('formats hours only', () => {
    expect(formatTime(3600)).toBe('1小时');
  });

  it('formats hours, minutes and seconds', () => {
    expect(formatTime(3665)).toBe('1小时1分5秒');
  });

  it('handles negative values', () => {
    expect(formatTime(-1)).toBe('0秒');
  });

  it('handles NaN', () => {
    expect(formatTime(NaN)).toBe('0秒');
  });

  it('handles Infinity', () => {
    expect(formatTime(Infinity)).toBe('0秒');
  });

  it('handles decimal seconds', () => {
    expect(formatTime(45.7)).toBe('45秒');
  });
});

describe('formatSize', () => {
  it('formats 0 bytes', () => {
    expect(formatSize(0)).toBe('0 B');
  });

  it('formats bytes under 1KB', () => {
    expect(formatSize(512)).toBe('512 B');
  });

  it('formats KB', () => {
    expect(formatSize(2048)).toBe('2.00 KB');
  });

  it('formats MB', () => {
    expect(formatSize(1048576)).toBe('1.00 MB');
  });

  it('formats GB', () => {
    expect(formatSize(1073741824)).toBe('1.00 GB');
  });

  it('handles negative values', () => {
    expect(formatSize(-1)).toBe('0 B');
  });

  it('handles NaN', () => {
    expect(formatSize(NaN)).toBe('0 B');
  });
});