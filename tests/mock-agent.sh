#!/bin/bash
# tests/mock-agent.sh
# Mock agent that simulates a Claude Code session.
# Reads GTR_AGENT and GTR_ROLE from environment.
# Responds to commands via stdin/stdout.

echo "Mock agent started: $GTR_AGENT ($GTR_ROLE)"
echo "Working directory: $(pwd)"
echo "Rig: $GTR_RIG"
echo "Work item: $GTR_WORK_ITEM"

# Simulate work
sleep 2

# Run gtr done if we have a work item
if [ -n "$GTR_WORK_ITEM" ]; then
    echo "Completing work item: $GTR_WORK_ITEM"
    # In a real test, would run: gtr done $GTR_WORK_ITEM
fi

echo "Mock agent exiting"
