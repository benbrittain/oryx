def _platforms_impl(ctx):
    constraints = dict()
    constraints.update(ctx.attrs.cpu_configuration[ConfigurationInfo].constraints)
    constraints.update(ctx.attrs.os_configuration[ConfigurationInfo].constraints)
    configuration = ConfigurationInfo(constraints = constraints, values = {})

    platform = ExecutionPlatformInfo(
        label = ctx.label.raw_target(),
        configuration = configuration,
        executor_config = CommandExecutorConfig(
            local_enabled = True,
            remote_enabled = ctx.attrs.remote,
            # NOTE limited hybrid only right now
            use_limited_hybrid = ctx.attrs.remote,
            remote_execution_properties = {},
            remote_execution_use_case = "buck2-default",
            remote_output_paths = "output_paths",
        ),
    )

    return [DefaultInfo(), ExecutionPlatformRegistrationInfo(platforms = [platform])]

default_config = struct(
  cpu = "prelude//cpu:x86_64",
  os = "prelude//os:linux",
)

platforms = rule(
    impl = _platforms_impl,
    attrs = {
      "remote": attrs.bool(default = False),
      "cpu_configuration": attrs.dep(providers = [ConfigurationInfo]),
      "os_configuration": attrs.dep(providers = [ConfigurationInfo]),
    },
)
