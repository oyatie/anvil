#!/usr/bin/env bash
dd bs=1 count=1 of=/dev/null 2>/dev/null
printf APPROVE
exit 0
