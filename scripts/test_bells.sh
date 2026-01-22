#!/bin/bash
# Test script for terminal bell functionality

echo "======================================"
echo "Terminal Bell Test Script"
echo "======================================"
echo ""
echo "IMPORTANT: Close the settings UI (F12) before running tests!"
echo ""
echo "This script will test various bell triggers."
echo "You should see/hear:"
echo "  - Visual bell: White flash on screen"
echo "  - Audio bell: Beep sound at configured volume"
echo "  - Desktop notification: System notification (if enabled)"
echo ""
echo "Check the terminal logs for detailed debugging output."
echo "Watch for:"
echo "  - 'Audio bell initialized successfully'"
echo "  - '🔔 Bell event detected'"
echo "  - 'Playing audio bell at X% volume'"
echo "  - 'Triggering visual bell flash'"
echo ""
echo "--------------------------------------"
echo "Test 1: ASCII BEL character (\\a)"
echo "--------------------------------------"
echo "Sending bell in 1 second..."
sleep 1
echo -e 'Bell now! \a'
sleep 2

echo ""
echo "--------------------------------------"
echo "Test 2: Ctrl+G equivalent (BEL)"
echo "--------------------------------------"
echo "Sending bell in 1 second..."
sleep 1
printf 'Bell now! \007\n'
sleep 2

echo ""
echo "--------------------------------------"
echo "Test 3: Multiple bells"
echo "--------------------------------------"
echo "Sending 3 bells in 1 second..."
sleep 1
echo -e 'Three bells! \a\a\a'
sleep 2

echo ""
echo "--------------------------------------"
echo "Test 4: Manual test - Type these commands:"
echo "--------------------------------------"
echo "Try these in the terminal:"
echo "  1. Type 'echo -e \"\\a\"' and press Enter"
echo "  2. Press Ctrl+G (should trigger bell)"
echo "  3. Press Tab in an empty line"
echo ""
echo "======================================"
echo "Automated tests complete!"
echo "======================================"
echo ""
echo "If you didn't see/hear anything:"
echo "1. Make sure settings UI is closed (F12)"
echo "2. Check that bells are enabled in settings"
echo "3. Look at terminal output for bell logs"
echo "4. Check if you see '🔔 Bell event detected'"
echo "5. Make sure audio is not muted"
echo "6. Try: RUST_LOG=debug cargo run"
