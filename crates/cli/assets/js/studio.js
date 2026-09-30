/**
 * Ruprizzle Studio page behaviour: error toasts, inline cell editing, dialogs,
 * the relation drawer and keyboard shortcuts.
 *
 * Plain DOM code on top of htmx. It replaces a hand-written "Alpine subset" that
 * crashed on its first selector (`[@click]` is not valid CSS), and that built
 * JavaScript source out of database values in `x-data`, so a cell containing a
 * quote could run code in the page.
 */
(function () {
  'use strict';

  function token() {
    return document.body.dataset.studioToken || '';
  }

  // ---------------------------------------------------------------- toasts --

  function toastHost() {
    let host = document.getElementById('toast-container');
    if (!host) {
      host = document.createElement('div');
      host.id = 'toast-container';
      host.className = 'toast-container';
      host.setAttribute('role', 'status');
      host.setAttribute('aria-live', 'polite');
      document.body.appendChild(host);
    }
    return host;
  }

  /** Shows a transient message. `kind` is `error`, `success` or `info`. */
  function toast(message, kind) {
    const el = document.createElement('div');
    el.className = 'toast toast-' + (kind || 'info');
    const text = document.createElement('span');
    text.textContent = message;
    const close = document.createElement('button');
    close.className = 'toast-close';
    close.type = 'button';
    close.setAttribute('aria-label', 'Dismiss');
    close.textContent = '×';
    close.addEventListener('click', () => el.remove());
    el.append(text, close);
    toastHost().appendChild(el);
    setTimeout(() => el.classList.add('toast-leaving'), kind === 'error' ? 8000 : 3500);
    setTimeout(() => el.remove(), kind === 'error' ? 8300 : 3800);
  }
  window.studioToast = toast;

  // htmx does not swap 4xx/5xx responses, so without this every refused write
  // failed silently.
  document.addEventListener('htmx:responseError', (e) => {
    const xhr = e.detail.xhr;
    const body = (xhr.responseText || '').trim();
    toast(body || 'Request failed with status ' + xhr.status + '.', 'error');
  });
  document.addEventListener('htmx:sendError', () => {
    toast('Studio did not respond. Is the server still running?', 'error');
  });

  // ------------------------------------------------------ inline editing --

  function restore(td, html) {
    td.classList.remove('editing');
    td.innerHTML = html;
    if (window.htmx) window.htmx.process(td);
  }

  function startEdit(td) {
    const original = td.innerHTML;
    const wasNull = td.dataset.null === 'true';
    const initial = wasNull ? '' : td.dataset.value || '';

    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'input-text cell-input';
    input.value = initial;
    input.setAttribute('aria-label', 'Edit ' + (td.dataset.column || 'cell'));
    if (td.dataset.optional === 'true') input.placeholder = 'Empty saves NULL';

    td.classList.add('editing');
    td.replaceChildren(input);
    input.focus();
    input.select();

    let settled = false;
    const cancel = () => {
      if (settled) return;
      settled = true;
      restore(td, original);
    };
    const commit = () => {
      if (settled) return;
      if (input.value === initial) {
        cancel();
        return;
      }
      settled = true;
      input.disabled = true;
      td.classList.add('saving');
      const done = () => {
        // A failed request is not swapped; put the old cell back.
        if (td.isConnected && td.contains(input)) {
          td.classList.remove('saving');
          restore(td, original);
        }
      };
      window.htmx
        .ajax('PATCH', td.dataset.patch, {
          source: td,
          target: td,
          swap: 'outerHTML',
          values: { value: input.value },
          headers: { 'x-studio-token': token() },
        })
        .then(done, done);
    };

    input.addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape') {
        ev.preventDefault();
        cancel();
      } else if (ev.key === 'Enter') {
        ev.preventDefault();
        commit();
      }
    });
    // Leaving an unchanged cell closes it; a changed one waits for Enter or Esc,
    // so a stray click never writes to the database.
    input.addEventListener('blur', () => {
      if (input.value === initial) cancel();
    });
  }

  document.addEventListener('dblclick', (e) => {
    const td = e.target.closest('td[data-editable]');
    if (td && !td.classList.contains('editing')) startEdit(td);
  });
  document.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter' || e.target.tagName !== 'TD') return;
    if (e.target.hasAttribute('data-editable')) {
      e.preventDefault();
      startEdit(e.target);
    }
  });

  // ------------------------------------------------------------- dialogs --

  document.addEventListener('click', (e) => {
    const opener = e.target.closest('[data-open-dialog]');
    if (opener) {
      const dialog = document.getElementById(opener.dataset.openDialog);
      if (dialog && !dialog.open) {
        dialog.showModal();
        const first = dialog.querySelector('input, select, textarea');
        if (first) first.focus();
      }
      return;
    }
    const closer = e.target.closest('[data-close-dialog]');
    if (closer) {
      const dialog = closer.closest('dialog');
      if (dialog) dialog.close();
      return;
    }
    // A click on the backdrop lands on the <dialog> element itself.
    if (e.target.tagName === 'DIALOG' && e.target.open) {
      const r = e.target.getBoundingClientRect();
      const inside =
        e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom;
      if (!inside) e.target.close();
    }
    if (e.target.closest('[data-close-drawer]')) closeDrawer();
  });

  // Close the insert dialog only once the row was actually written.
  document.addEventListener('htmx:afterRequest', (e) => {
    const form = e.detail.elt;
    if (!(form instanceof HTMLFormElement) || !form.hasAttribute('data-reset-on-success')) return;
    if (!e.detail.successful) return;
    form.reset();
    const dialog = form.closest('dialog');
    if (dialog) dialog.close();
    const empty = document.getElementById('empty-row');
    if (empty) empty.remove();
    toast(form.dataset.successMessage || 'Saved.', 'success');
  });

  document.addEventListener('htmx:afterSwap', (e) => {
    if (e.detail.target && e.detail.target.id === 'relation-drawer') {
      const btn = e.detail.target.querySelector('[data-close-drawer]');
      if (btn) btn.focus();
    }
  });

  function closeDrawer() {
    const drawer = document.getElementById('relation-drawer');
    if (drawer) drawer.replaceChildren();
  }

  // ----------------------------------------------------------- shortcuts --

  function typing(el) {
    return el && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName));
  }

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      const drawer = document.getElementById('relation-drawer');
      if (drawer && drawer.childElementCount) {
        closeDrawer();
        return;
      }
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter' && e.target.matches('textarea[data-submit-on-ctrl-enter]')) {
      e.preventDefault();
      const form = e.target.closest('form');
      if (form) form.requestSubmit();
      return;
    }
    if (e.key === '/' && !typing(document.activeElement)) {
      const search = document.querySelector('[data-search-shortcut]') || document.getElementById('nav-filter');
      if (search) {
        e.preventDefault();
        search.focus();
        search.select();
      }
    }
  });

  // -------------------------------------------------------- sidebar filter --

  document.addEventListener('input', (e) => {
    if (e.target.id !== 'nav-filter') return;
    const term = e.target.value.trim().toLowerCase();
    let visible = 0;
    document.querySelectorAll('.nav-item[data-model]').forEach((item) => {
      const show = !term || item.dataset.model.toLowerCase().includes(term);
      item.hidden = !show;
      if (show) visible += 1;
    });
    const none = document.getElementById('nav-empty');
    if (none) none.hidden = visible !== 0;
  });
})();
