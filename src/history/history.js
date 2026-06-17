// History window - dedicated translation history viewer

const { invoke } = window.__TAURI__.core;

// Engine labels (same as settings)
const ENGINE_LABELS = {
    google_gtx: 'Google Translate',
    mymemory: 'MyMemory',
    gemini: 'Gemini',
    deepl: 'DeepL',
    deepseek: 'DeepSeek',
    groq: 'Groq',
};

// DOM elements
const historySearchInput = document.getElementById('history-search');
const historyList = document.getElementById('history-list');
const historyLoadMoreBtn = document.getElementById('history-load-more');
const historyExportJsonBtn = document.getElementById('history-export-json');
const historyExportCsvBtn = document.getElementById('history-export-csv');
const historyClearBtn = document.getElementById('history-clear');
const emptyStateEl = document.getElementById('empty-state');
const messageEl = document.getElementById('message');
const closeBtn = document.getElementById('close-btn');

// State
let historyOffset = 0;
let historyHasMore = true;

// Show message toast
function showMessage(text, isError = false) {
    messageEl.textContent = text;
    messageEl.className = isError ? 'error' : 'success';
    setTimeout(() => { messageEl.className = ''; }, 3000);
}

// Format a timestamp string for display (e.g. "2024-01-01 14:30:45" -> "14:30:45")
function formatTimestamp(ts) {
    if (!ts) return '';
    // If timestamp contains a space, extract just the time part
    const parts = ts.split(' ');
    if (parts.length >= 2) return parts[1];
    // If it's just a time string, return as-is
    if (ts.includes(':')) return ts;
    return ts;
}

// Render a single history entry (terminal style)
function renderHistoryEntry(entry) {
    const div = document.createElement('div');
    div.className = 'history-entry';
    div.dataset.id = entry.id;

    const engineLabel = ENGINE_LABELS[entry.engine] || entry.engine || 'unknown';

    const prefixSpan = document.createElement('span');
    prefixSpan.className = 'entry-prefix';
    prefixSpan.textContent = '> ';

    const originalSpan = document.createElement('span');
    originalSpan.className = 'entry-original';
    originalSpan.textContent = '"' + entry.original_text + '"';

    const arrowSpan = document.createElement('span');
    arrowSpan.className = 'entry-arrow';
    arrowSpan.textContent = ' → ';

    const translatedSpan = document.createElement('span');
    translatedSpan.className = 'entry-translated';
    translatedSpan.textContent = '"' + entry.translated_text + '"';

    const metaSpan = document.createElement('span');
    metaSpan.className = 'entry-meta';
    const ts = formatTimestamp(entry.created_at);
    metaSpan.textContent = ' [' + engineLabel + '] ' + (ts ? ts + ' ' : '');

    div.appendChild(prefixSpan);
    div.appendChild(originalSpan);
    div.appendChild(arrowSpan);
    div.appendChild(translatedSpan);
    div.appendChild(metaSpan);

    // Profile label if present
    if (entry.profile_id) {
        const profileSpan = document.createElement('span');
        profileSpan.className = 'entry-profile';
        profileSpan.textContent = '[' + entry.profile_id + '] ';
        div.appendChild(profileSpan);
    }

    const deleteBtn = document.createElement('button');
    deleteBtn.className = 'delete-entry-btn';
    deleteBtn.dataset.id = entry.id;
    deleteBtn.textContent = '[delete]';
    div.appendChild(deleteBtn);

    // Delete button handler
    deleteBtn.addEventListener('click', async (e) => {
        e.stopPropagation();
        const id = parseInt(e.target.dataset.id);
        if (confirm('Delete this history entry?')) {
            try {
                await invoke('delete_history_entry', { id });
                historyOffset = 0;
                await renderHistory();
            } catch (err) {
                showMessage('Failed to delete: ' + err, true);
            }
        }
    });

    return div;
}

// Render history list (paginated)
async function renderHistory() {
    try {
        const limit = 20;
        const entries = await invoke('get_history', { limit, offset: historyOffset });

        if (historyOffset === 0) {
            historyList.innerHTML = '';
        }

        if (entries.length === 0 && historyOffset === 0) {
            emptyStateEl.style.display = 'block';
            historyList.style.display = 'none';
            historyHasMore = false;
        } else {
            emptyStateEl.style.display = 'none';
            historyList.style.display = 'block';
            entries.forEach(entry => {
                historyList.appendChild(renderHistoryEntry(entry));
            });
            historyHasMore = entries.length >= limit;
        }

        historyLoadMoreBtn.style.display = historyHasMore ? 'inline-block' : 'none';
    } catch (err) {
        historyList.innerHTML = '<div style="color: var(--error); padding: 8px;">Failed to load history: ' + (err || 'unknown') + '</div>';
        historyHasMore = false;
    }
}

// Search history with debounce
let searchTimeout = null;
function searchHistory() {
    const query = historySearchInput.value.trim();

    if (!query) {
        historyOffset = 0;
        renderHistory();
        return;
    }

    historyLoadMoreBtn.style.display = 'none';

    invoke('search_history', { query }).then(entries => {
        historyList.innerHTML = '';
        emptyStateEl.style.display = 'none';
        historyList.style.display = 'block';
        if (entries.length === 0) {
            historyList.innerHTML = '<div style="color: var(--text-secondary); padding: 8px; font-family: var(--font-mono);">No matches found</div>';
        } else {
            entries.forEach(entry => {
                historyList.appendChild(renderHistoryEntry(entry));
            });
        }
        historyHasMore = false;
    }).catch(err => {
        showMessage('Search failed: ' + err, true);
    });
}

// Export history
async function exportHistory(format) {
    try {
        const data = await invoke('export_history', { format });

        // Create download
        const blob = new Blob([data], { type: format === 'json' ? 'application/json' : 'text/csv' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `overlex-history.${format}`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);

        showMessage('Export complete');
    } catch (err) {
        showMessage('Export failed: ' + err, true);
    }
}

// Clear all history
async function clearHistory() {
    if (!confirm('Are you sure you want to delete ALL translation history? This cannot be undone.')) {
        return;
    }

    try {
        await invoke('clear_history');
        historyOffset = 0;
        await renderHistory();
        showMessage('History cleared');
    } catch (err) {
        showMessage('Failed to clear history: ' + err, true);
    }
}

// Event listeners
historySearchInput.addEventListener('input', () => {
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(searchHistory, 300);
});

historyLoadMoreBtn.addEventListener('click', async () => {
    historyOffset += 20;
    await renderHistory();
});

historyExportJsonBtn.addEventListener('click', () => exportHistory('json'));
historyExportCsvBtn.addEventListener('click', () => exportHistory('csv'));
historyClearBtn.addEventListener('click', clearHistory);

// Close button
closeBtn.addEventListener('click', async () => {
    try {
        await invoke('hide_window', { label: 'history' });
    } catch (err) {
        console.error('Failed to hide history window:', err);
    }
});

// ESC key to close
document.addEventListener('keydown', async (e) => {
    if (e.key === 'Escape') {
        try {
            await invoke('hide_window', { label: 'history' });
        } catch (err) {
            console.error('Failed to hide history window:', err);
        }
    }
});

// Window drag via title bar
const titleBar = document.querySelector('.window-titlebar');
if (titleBar) {
    titleBar.addEventListener('mousedown', async (e) => {
        if (e.target.closest('.window-btn')) return;
        try {
            const win = window.__TAURI__.window.getCurrentWindow();
            await win.startDragging();
        } catch (err) {
            console.error('Failed to start dragging:', err);
        }
    });
}

// Load history on startup
document.addEventListener('DOMContentLoaded', () => {
    renderHistory();
});
