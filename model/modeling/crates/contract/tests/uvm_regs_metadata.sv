// Standalone real-UVM metadata oracle. No RTL or mirrored-state assumptions.
module pdma_uvm_regs_metadata;
  import uvm_pkg::*;
  import pdma_uvm_regs::*;
  task automatic check(input int unsigned channels, input int unsigned bus_bytes);
    DmaRegisters model;
    uvm_reg_addr_t base;
    uvm_reg_addr_t channel_start;
    uvm_reg all_regs[$];
    base = 'h20000;
    channel_start = ((20 + 4*channels + 4095)/4096)*4096;
    model = new($sformatf("model_%0d_%0d", channels, bus_bytes));
    model.build(channels, bus_bytes);
    model.default_map.set_base_addr(base);
    model.lock_model();
    model.get_registers(all_regs);
    if (all_regs.size() != 5 + 18 * channels) $fatal(1, "register count");
    if (model.channels_block.size() != channels) $fatal(1, "channel bank size");
    if (model.admin_block.channels_block.size() != channels) $fatal(1, "admin bank size");
    if (model.default_map.get_n_bytes() != bus_bytes) $fatal(1, "bus width");
    if (model.default_map.get_endian() != UVM_LITTLE_ENDIAN) $fatal(1, "endian");
    if (model.discovery_block.channel_num_reg.value_field.has_reset()) $fatal(1, "PostReset known");
    if (model.discovery_block.capabilities_reg.atomic64_field.get_lsb_pos() != 1) $fatal(1, "field position");
    if (model.admin_block.control_block.reset_reg.request_field.get_n_bits() != 1) $fatal(1, "field width");
    if (!model.admin_block.control_block.reset_reg.request_field.is_volatile()) $fatal(1, "volatility");
    if (model.admin_block.control_block.reset_reg.request_field.get_compare() != UVM_NO_CHECK) $fatal(1, "compare policy");
    foreach (model.channels_block[i]) begin
      if (model.admin_block.channels_block[i].reset_reg.get_address() != base + 20 + i*4) $fatal(1, "admin address");
      if (model.channels_block[i].command_block.base_reg.get_address() != base + channel_start + 4096*i) $fatal(1, "command address");
      if (model.channels_block[i].event_block.base_reg.get_address() != base + channel_start + 4096*i + 24) $fatal(1, "event address");
      if (model.channels_block[i].control_block.irq_block.trap_ack_reg.get_address() != base + channel_start + 4096*i + 76) $fatal(1, "nested address");
      if (model.channels_block[i].command_block.base_reg.get_n_bits() != 64) $fatal(1, "address width");
      if (model.channels_block[i].command_block.capacity_reg.value_field.get_n_bits() != 32) $fatal(1, "word width");
      if (model.channels_block[i].command_block.consumer_reg.value_field.get_access() != "RO") $fatal(1, "read-only");
      if (model.channels_block[i].command_block.producer_reg.value_field.get_access() != "RW") $fatal(1, "read-write");
      if (!model.channels_block[i].command_block.base_reg.value_field.has_reset()) $fatal(1, "reset missing");
      if (model.channels_block[i].command_block.base_reg.value_field.get_reset() != 0) $fatal(1, "reset value");
      if (model.channels_block[i].command_block.default_map.get_n_bytes() != bus_bytes) $fatal(1, "nested bus width");
    end
    $display("RAL_METADATA_PASS channels=%0d bus_bytes=%0d", channels, bus_bytes);
  endtask
  initial begin
    if (DmaFatal::CODE_INTERNAL != 1 || DmaFatal::CODE_NONE != 0)
      $fatal(1, "enum value metadata");
    check(1, 4);
    check(3, 4);
    check(1, 8);
    check(3, 8);
    check(1020, 4);
    $finish;
  end
endmodule
