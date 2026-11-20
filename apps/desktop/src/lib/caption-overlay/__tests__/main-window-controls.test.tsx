import { createI18n, I18nextProvider } from '@sonasub/i18n';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import { FloatingCaptionButton } from '@/components/floating-caption-button';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

const { listen } = vi.hoisted(() => ({ listen: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));
listen.mockImplementation(() => Promise.resolve(() => undefined));

afterEach(() => {
  cleanup();
  invoke.mockReset();
  listen.mockReset();
  listen.mockImplementation(() => Promise.resolve(() => undefined));
});

function renderButton() {
  render(
    <I18nextProvider i18n={createI18n('en-US')}>
      <FloatingCaptionButton recordId="record-1" />
    </I18nextProvider>,
  );
}

describe('FloatingCaptionButton', () => {
  it('opens the overlay for the selected record', () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'caption_overlay_lock_state') {
        return Promise.resolve({ locked: false, isOpen: false });
      }
      return Promise.resolve(undefined);
    });
    renderButton();

    fireEvent.click(screen.getByRole('button', { name: 'Open floating captions' }));

    expect(invoke).toHaveBeenCalledWith('open_caption_overlay', { recordId: 'record-1' });
  });

  it('unlocks the overlay from the main window', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'caption_overlay_lock_state') {
        return Promise.resolve({ locked: true, isOpen: true });
      }
      return Promise.resolve(undefined);
    });
    renderButton();
    await screen.findByRole('button', { name: 'Unlock floating captions' });

    fireEvent.click(screen.getByRole('button', { name: 'Unlock floating captions' }));

    expect(invoke).toHaveBeenCalledWith('set_caption_overlay_locked', { locked: false });
  });

  it('closes the overlay without stopping capture', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'caption_overlay_lock_state') {
        return Promise.resolve({ locked: false, isOpen: true });
      }
      return Promise.resolve(undefined);
    });
    renderButton();
    await screen.findByRole('button', { name: 'Open floating captions' });

    fireEvent.click(screen.getByRole('button', { name: 'Close floating captions' }));

    expect(invoke).toHaveBeenCalledWith('close_caption_overlay');
    expect(invoke).not.toHaveBeenCalledWith('stop_record', expect.anything());
  });
});
