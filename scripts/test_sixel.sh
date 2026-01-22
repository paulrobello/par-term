#!/bin/bash
# Test script to display a simple sixel graphic in the terminal

echo "Testing Sixel Graphics Support"
echo "This should display a red and blue square pattern:"
echo ""

# Simple 10x10 red and blue checkerboard
# ESC P 0 ; 0 ; 0 q = Start sixel (P0 = DCS, q = sixel mode)
# "1;2;100;100 = Raster attributes (1:1 aspect, 100x100 pixels)
# #0;2;100;0;0 = Define color 0 as red (RGB 100,0,0)
# #1;2;0;0;100 = Define color 1 as blue (RGB 0,0,100)
# #0!100~ = Red, repeat 100 times
# $- = New line
# #1!100~ = Blue, repeat 100 times
# ESC \ = End sixel

printf '\033P0;0;0q"1;2;100;100#0;2;100;0;0#1;2;0;0;100#0!100~$-#1!100~$-#0!100~$-#1!100~$-#0!100~$-#1!100~$-#0!100~$-#1!100~$-#0!100~$-#1!100~\033\\'

echo ""
echo "Did you see the graphic?"
