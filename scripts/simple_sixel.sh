#!/bin/bash
# Minimal sixel test - single red pixel

echo "Minimal sixel test:"
printf '\033Pq#0;2;100;0;0#0~\033\\'
echo ""
echo "Done"
