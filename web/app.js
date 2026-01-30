// hashcards - Static Site JavaScript

import init, { HashcardsApp, now_timestamp, today_date } from './pkg/hashcards_wasm.js';

let app = null;
let macros = {};

// Screen management
function showScreen(screenId) {
    document.querySelectorAll('.screen').forEach(screen => {
        screen.style.display = 'none';
    });
    document.getElementById(screenId).style.display = 'flex';
}

function setLoadingStatus(status) {
    document.getElementById('loading-status').textContent = status;
}

// Initialize the application
async function initialize() {
    try {
        setLoadingStatus('Loading WASM module...');
        await init();

        setLoadingStatus('Initializing application...');
        app = new HashcardsApp();

        setupEventListeners();
        showScreen('setup-screen');
    } catch (error) {
        console.error('Failed to initialize:', error);
        setLoadingStatus('Failed to load: ' + error.message);
    }
}

// Setup event listeners
function setupEventListeners() {
    // File upload
    const uploadArea = document.getElementById('upload-area');
    const fileInput = document.getElementById('file-input');
    const selectFolderBtn = document.getElementById('select-folder-btn');

    selectFolderBtn.addEventListener('click', () => fileInput.click());

    fileInput.addEventListener('change', (e) => handleFiles(e.target.files));

    // Drag and drop
    uploadArea.addEventListener('dragover', (e) => {
        e.preventDefault();
        uploadArea.classList.add('drag-over');
    });

    uploadArea.addEventListener('dragleave', () => {
        uploadArea.classList.remove('drag-over');
    });

    uploadArea.addEventListener('drop', (e) => {
        e.preventDefault();
        uploadArea.classList.remove('drag-over');
        handleFiles(e.dataTransfer.files);
    });

    // Session controls
    document.getElementById('start-session-btn').addEventListener('click', startSession);
    document.getElementById('new-session-btn').addEventListener('click', startSession);
    document.getElementById('back-to-setup-btn').addEventListener('click', () => showScreen('setup-screen'));

    // Card controls
    document.getElementById('reveal-btn').addEventListener('click', revealCard);
    document.getElementById('forgot-btn').addEventListener('click', () => gradeCard('forgot'));
    document.getElementById('hard-btn').addEventListener('click', () => gradeCard('hard'));
    document.getElementById('good-btn').addEventListener('click', () => gradeCard('good'));
    document.getElementById('easy-btn').addEventListener('click', () => gradeCard('easy'));

    // Keyboard shortcuts
    document.addEventListener('keydown', handleKeydown);

    // Import/Export
    document.getElementById('export-btn').addEventListener('click', exportProgress);
    document.getElementById('import-btn').addEventListener('click', () => {
        document.getElementById('import-input').click();
    });
    document.getElementById('import-input').addEventListener('change', importProgress);
}

// Handle file selection
async function handleFiles(files) {
    const mdFiles = [];
    const mediaFiles = new Map();

    // Process all files
    for (const file of files) {
        const path = file.webkitRelativePath || file.name;

        if (path.endsWith('.md')) {
            try {
                const content = await file.text();
                mdFiles.push([path, content]);
            } catch (error) {
                console.error('Failed to read file:', path, error);
            }
        } else if (isMediaFile(path)) {
            // Create blob URL for media files
            const blobUrl = URL.createObjectURL(file);
            mediaFiles.set(path, blobUrl);
        }
    }

    if (mdFiles.length === 0) {
        alert('No markdown files found. Please select a folder containing .md files.');
        return;
    }

    try {
        // Load cards
        const filesJson = JSON.stringify(mdFiles);
        const cardCount = app.load_cards(filesJson);

        // Register media files
        for (const [path, blobUrl] of mediaFiles) {
            // Register various path formats
            const relativePath = path.split('/').slice(1).join('/'); // Remove first directory
            app.register_media(path, blobUrl);
            app.register_media(relativePath, blobUrl);
            app.register_media('@/' + relativePath, blobUrl);

            // Also register just the filename
            const filename = path.split('/').pop();
            app.register_media(filename, blobUrl);
        }

        // Update UI
        const deckNames = JSON.parse(app.deck_names());
        document.getElementById('card-count').textContent = cardCount;
        document.getElementById('deck-count').textContent = deckNames.length;

        // Calculate due today
        const dueCount = calculateDueToday();
        document.getElementById('due-count').textContent = dueCount;

        document.getElementById('collection-info').style.display = 'block';
        document.getElementById('upload-area').style.display = 'none';
    } catch (error) {
        console.error('Failed to load cards:', error);
        alert('Failed to load cards: ' + error);
    }
}

function isMediaFile(path) {
    const ext = path.split('.').pop().toLowerCase();
    return ['png', 'jpg', 'jpeg', 'gif', 'svg', 'mp3', 'wav', 'ogg', 'mp4', 'webm'].includes(ext);
}

function calculateDueToday() {
    // This is a simple estimate - actual due count determined when session starts
    return app.collection_size();
}

// Start a drilling session
function startSession() {
    const shuffle = document.getElementById('shuffle-cards').checked;
    const cardLimitInput = document.getElementById('card-limit').value;
    const newCardLimitInput = document.getElementById('new-card-limit').value;

    const cardLimit = cardLimitInput ? parseInt(cardLimitInput) : null;
    const newCardLimit = newCardLimitInput ? parseInt(newCardLimitInput) : null;

    try {
        const today = today_date();
        const dueCount = app.start_session(today, shuffle, cardLimit, newCardLimit);

        if (dueCount === 0) {
            alert('No cards due today!');
            return;
        }

        showScreen('drill-screen');
        renderCard();
    } catch (error) {
        console.error('Failed to start session:', error);
        alert('Failed to start session: ' + error);
    }
}

// Render the current card
function renderCard() {
    if (!app.has_cards()) {
        finishSession();
        return;
    }

    const deckName = app.current_deck_name() || 'Unknown Deck';
    document.getElementById('deck-name').textContent = deckName;

    try {
        const frontHtml = app.current_front_html();
        document.getElementById('question').innerHTML = frontHtml;

        if (app.is_revealed()) {
            const backHtml = app.current_back_html();
            document.getElementById('answer').innerHTML = backHtml;
            document.getElementById('answer').style.display = 'block';
            document.getElementById('reveal-controls').style.display = 'none';
            document.getElementById('grade-controls').style.display = 'flex';
        } else {
            document.getElementById('answer').style.display = 'none';
            document.getElementById('reveal-controls').style.display = 'flex';
            document.getElementById('grade-controls').style.display = 'none';
        }

        // Update progress
        const total = app.total_cards();
        const remaining = app.remaining_cards();
        const reviewed = total - remaining;
        const progress = app.progress() * 100;

        document.getElementById('progress-fill').style.width = progress + '%';
        document.getElementById('progress-text').textContent = reviewed + ' / ' + total;

        // Render math
        renderMath();

        // Make card content visible
        document.getElementById('card-content').style.opacity = '1';
    } catch (error) {
        console.error('Failed to render card:', error);
    }
}

// Reveal the answer
function revealCard() {
    app.reveal();
    renderCard();
}

// Grade the current card
function gradeCard(grade) {
    try {
        const now = now_timestamp();
        app.grade_card(grade, now);
        renderCard();
    } catch (error) {
        console.error('Failed to grade card:', error);
        alert('Failed to grade card: ' + error);
    }
}

// Finish the session
function finishSession() {
    document.getElementById('reviewed-count').textContent = app.total_cards();
    showScreen('finished-screen');
}

// Keyboard handling
function handleKeydown(event) {
    // Ignore if typing in an input
    if (event.target.tagName === 'INPUT' || event.target.tagName === 'TEXTAREA') {
        return;
    }

    // Ignore with modifiers
    if (event.shiftKey || event.ctrlKey || event.altKey || event.metaKey) {
        return;
    }

    const drillScreen = document.getElementById('drill-screen');
    if (drillScreen.style.display === 'none') {
        return;
    }

    switch (event.key) {
        case ' ':
            event.preventDefault();
            if (!app.is_revealed()) {
                revealCard();
            }
            break;
        case '1':
            if (app.is_revealed()) gradeCard('forgot');
            break;
        case '2':
            if (app.is_revealed()) gradeCard('hard');
            break;
        case '3':
            if (app.is_revealed()) gradeCard('good');
            break;
        case '4':
            if (app.is_revealed()) gradeCard('easy');
            break;
    }
}

// Render math with KaTeX
function renderMath() {
    if (typeof renderMathInElement === 'function') {
        renderMathInElement(document.getElementById('card-content'), {
            delimiters: [
                { left: '$$', right: '$$', display: true },
                { left: '$', right: '$', display: false },
                { left: '\\(', right: '\\)', display: false },
                { left: '\\[', right: '\\]', display: true },
            ],
            macros: macros,
            throwOnError: false,
        });
    }
}

// Export progress
function exportProgress() {
    const data = app.export_performance();
    const blob = new Blob([data], { type: 'application/json' });
    const url = URL.createObjectURL(blob);

    const a = document.createElement('a');
    a.href = url;
    a.download = 'hashcards-progress.json';
    a.click();

    URL.revokeObjectURL(url);
}

// Import progress
function importProgress(event) {
    const file = event.target.files[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (e) => {
        try {
            app.import_performance(e.target.result);
            alert('Progress imported successfully!');
        } catch (error) {
            console.error('Failed to import progress:', error);
            alert('Failed to import progress: ' + error);
        }
    };
    reader.readAsText(file);
}

// Start the application
initialize();
