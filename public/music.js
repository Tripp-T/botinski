(() => {
  const selected = new Set();

  function updateBulkUI() {
    const count = selected.size;
    document.querySelectorAll('.selected-count').forEach(el => el.textContent = count);
    document.querySelectorAll('.bulk-action').forEach(btn => btn.disabled = count === 0);
  }

  function reapplySelection() {
    const present = new Set();
    document.querySelectorAll('.track-checkbox').forEach(cb => {
      const id = cb.dataset.trackId;
      present.add(id);
      cb.checked = selected.has(id);
    });
    for (const id of [...selected]) if (!present.has(id)) selected.delete(id);
    updateBulkUI();
  }

  document.addEventListener('change', e => {
    const cb = e.target.closest('.track-checkbox');
    if (!cb) return;
    const id = cb.dataset.trackId;
    if (cb.checked) selected.add(id); else selected.delete(id);
    updateBulkUI();
  });

  document.addEventListener('click', e => {
    if (e.target.closest('[data-bulk-select-all]')) {
      document.querySelectorAll('.track-checkbox').forEach(cb => {
        cb.checked = true;
        selected.add(cb.dataset.trackId);
      });
      updateBulkUI();
    } else if (e.target.closest('[data-bulk-deselect-all]')) {
      selected.clear();
      document.querySelectorAll('.track-checkbox').forEach(cb => cb.checked = false);
      updateBulkUI();
    }
  });

  document.body.addEventListener('htmx:configRequest', e => {
    const trig = e.detail.elt;
    if (trig && trig.classList && trig.classList.contains('bulk-action')) {
      e.detail.parameters['ids'] = [...selected].join(',');
    }
  });

  document.body.addEventListener('htmx:afterSwap', reapplySelection);
})();
