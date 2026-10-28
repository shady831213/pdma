# DMA hardware

Own the direct DMA's channel state, shared command execution, register effects
and AXI interfaces. Channels share execution capacity; they are not independent
copies of the engine. Channel state and stage-local state must have explicit owners.

Function ownership, managed access windows and context-table resolution belong
to the sibling `hw_components` directory. No RTL modules are implemented yet;
internal directories will follow the reviewed hardware structure.
