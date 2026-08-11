import 'package:flutter/material.dart';
import 'package:gixgiz_desktop/app/app_keys.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_state.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';

class RuntimePanel extends StatelessWidget {
  const RuntimePanel({
    required this.state,
    required this.inventoryState,
    this.onRefresh,
    this.onApproveReuse,
    this.onStartOperation,
    this.onCancelOperation,
    this.onToggleModels,
    this.primaryActionFocusNode,
    super.key,
  });

  final RuntimeStatusState state;
  final RuntimeInventoryState inventoryState;
  final VoidCallback? onRefresh;
  final VoidCallback? onApproveReuse;
  final ValueChanged<RuntimeOperationKind>? onStartOperation;
  final VoidCallback? onCancelOperation;
  final VoidCallback? onToggleModels;
  final FocusNode? primaryActionFocusNode;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final report = _reportFor(state);
    final status = _statusFor(localizations, state);
    final ownership = _ownershipLabel(localizations, report?.ownership);
    final reuseConsent = _consentLabel(localizations, report?.reuseConsent);
    final requiresManagementConsent =
        report != null &&
        _hasAvailability(
          report,
          RuntimeCapabilityAvailability.requiresManagementConsent,
        );
    final statusColor = switch (status.tone) {
      _RuntimeTone.ready => Theme.of(context).colorScheme.primary,
      _RuntimeTone.warning => Theme.of(context).colorScheme.tertiary,
      _RuntimeTone.error => Theme.of(context).colorScheme.error,
      _RuntimeTone.neutral => Theme.of(context).colorScheme.secondary,
    };
    final diagnosticCode = switch (state) {
      RuntimeStatusFailed(:final diagnosticCode) => diagnosticCode,
      _ => switch (inventoryState) {
        RuntimeInventoryFailed(:final diagnosticCode) => diagnosticCode,
        _ => null,
      },
    };
    final recoveryAction = switch (state) {
      RuntimeStatusFailed(:final recoveryAction) => recoveryAction,
      _ => switch (inventoryState) {
        RuntimeInventoryFailed(:final recoveryAction) => recoveryAction,
        _ => null,
      },
    };

    return Card(
      key: AppKeys.runtimePanel,
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              localizations.runtimeTitle,
              style: Theme.of(context).textTheme.titleLarge,
            ),
            const SizedBox(height: 16),
            Semantics(
              key: AppKeys.runtimeStatus,
              container: true,
              liveRegion: true,
              label: localizations.runtimeStatusSemanticLabel(
                status.title,
                status.message,
                ownership,
                reuseConsent,
              ),
              child: ExcludeSemantics(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Icon(status.icon, color: statusColor, size: 28),
                    const SizedBox(width: 16),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            status.title,
                            style: Theme.of(context).textTheme.titleMedium,
                          ),
                          const SizedBox(height: 6),
                          Text(status.message),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
            if (state is RuntimeStatusLoading ||
                state is RuntimeStatusOperating) ...[
              const SizedBox(height: 20),
              Semantics(
                key: AppKeys.runtimeProgress,
                label: localizations.runtimeProgressSemanticLabel,
                value: localizations.inProgressSemanticValue,
                child: const LinearProgressIndicator(),
              ),
            ],
            if (report != null) ...[
              const SizedBox(height: 20),
              const Divider(),
              const SizedBox(height: 12),
              _RuntimeDetails(report: report),
              if (report.ownership == RuntimeOwnership.external) ...[
                const SizedBox(height: 16),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Icon(Icons.info_outline, size: 20),
                    const SizedBox(width: 8),
                    Expanded(child: Text(localizations.runtimeExternalNotice)),
                  ],
                ),
              ],
              if (requiresManagementConsent) ...[
                const SizedBox(height: 16),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Icon(Icons.admin_panel_settings_outlined, size: 20),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        report.ownership == RuntimeOwnership.external
                            ? localizations
                                  .runtimeExternalManagementRequiredNotice
                            : localizations.runtimeManagementRequiredNotice,
                      ),
                    ),
                  ],
                ),
              ],
              if (report.reasons.isNotEmpty) ...[
                const SizedBox(height: 16),
                _MessageList(
                  title: localizations.runtimeReasonsLabel,
                  messages: report.reasons.map(
                    (item) => _reasonLabel(localizations, item.code),
                  ),
                  icon: Icons.info_outline,
                ),
              ],
              if (report.warnings.isNotEmpty) ...[
                const SizedBox(height: 12),
                _MessageList(
                  title: localizations.runtimeWarningsLabel,
                  messages: report.warnings.map(
                    (item) => _warningLabel(localizations, item.code),
                  ),
                  icon: Icons.warning_amber,
                ),
              ],
            ],
            const SizedBox(height: 20),
            _RuntimeActions(
              state: state,
              report: report,
              inventoryState: inventoryState,
              onRefresh: onRefresh,
              onApproveReuse: onApproveReuse,
              onStartOperation: onStartOperation,
              onCancelOperation: onCancelOperation,
              onToggleModels: onToggleModels,
              primaryActionFocusNode: primaryActionFocusNode,
            ),
            if (inventoryState is! RuntimeInventoryIdle) ...[
              const SizedBox(height: 20),
              const Divider(),
              const SizedBox(height: 12),
              _RuntimeInventory(state: inventoryState),
            ],
            if (recoveryAction != null) ...[
              const SizedBox(height: 12),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Icon(Icons.tips_and_updates_outlined, size: 20),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      localizations.runtimeRecommendedAction(
                        _recoveryActionLabel(localizations, recoveryAction),
                      ),
                    ),
                  ),
                ],
              ),
            ],
            if (diagnosticCode != null) ...[
              const SizedBox(height: 12),
              ExpansionTile(
                key: AppKeys.runtimeDiagnostics,
                tilePadding: EdgeInsets.zero,
                childrenPadding: const EdgeInsets.only(bottom: 8),
                title: Text(localizations.diagnosticsLabel),
                children: [
                  Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: SelectableText(
                      localizations.diagnosticCodeLabel(diagnosticCode),
                    ),
                  ),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _RuntimeActions extends StatelessWidget {
  const _RuntimeActions({
    required this.state,
    required this.report,
    required this.inventoryState,
    required this.onRefresh,
    required this.onApproveReuse,
    required this.onStartOperation,
    required this.onCancelOperation,
    required this.onToggleModels,
    required this.primaryActionFocusNode,
  });

  final RuntimeStatusState state;
  final RuntimeHealthReport? report;
  final RuntimeInventoryState inventoryState;
  final VoidCallback? onRefresh;
  final VoidCallback? onApproveReuse;
  final ValueChanged<RuntimeOperationKind>? onStartOperation;
  final VoidCallback? onCancelOperation;
  final VoidCallback? onToggleModels;
  final FocusNode? primaryActionFocusNode;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final busy =
        state is RuntimeStatusLoading ||
        state is RuntimeStatusOperating ||
        inventoryState is RuntimeInventoryLoading;
    final actionableReport = state is RuntimeStatusLoaded ? report : null;
    final allowReuse =
        actionableReport != null &&
        _hasAvailability(
          actionableReport,
          RuntimeCapabilityAvailability.requiresReuseConsent,
        );
    final canStart = _isAvailable(
      actionableReport,
      RuntimeCapabilityKind.start,
    );
    final canStop = _isAvailable(actionableReport, RuntimeCapabilityKind.stop);
    final canRestart = _isAvailable(
      actionableReport,
      RuntimeCapabilityKind.restart,
    );
    final canListModels = _isAvailable(
      actionableReport,
      RuntimeCapabilityKind.modelInventory,
    );

    return Wrap(
      spacing: 12,
      runSpacing: 12,
      children: [
        OutlinedButton.icon(
          key: AppKeys.runtimeRefreshAction,
          focusNode: primaryActionFocusNode,
          style: _outlinedActionStyle(context),
          onPressed: busy ? null : onRefresh,
          icon: const Icon(Icons.refresh),
          label: Text(localizations.runtimeRefreshAction),
        ),
        if (allowReuse)
          FilledButton.icon(
            key: AppKeys.runtimeConsentAction,
            onPressed: busy || onApproveReuse == null
                ? null
                : () => _confirmReuse(context),
            icon: const Icon(Icons.check_circle_outline),
            label: Text(localizations.runtimeConsentApproveAction),
          ),
        if (canStart)
          FilledButton.icon(
            key: AppKeys.runtimeStartAction,
            onPressed: busy || onStartOperation == null
                ? null
                : () => onStartOperation!(RuntimeOperationKind.start),
            icon: const Icon(Icons.play_arrow),
            label: Text(localizations.runtimeStartAction),
          ),
        if (canStop)
          OutlinedButton.icon(
            key: AppKeys.runtimeStopAction,
            style: _outlinedActionStyle(context),
            onPressed: busy || onStartOperation == null
                ? null
                : () => onStartOperation!(RuntimeOperationKind.stop),
            icon: const Icon(Icons.stop),
            label: Text(localizations.runtimeStopAction),
          ),
        if (canRestart)
          OutlinedButton.icon(
            key: AppKeys.runtimeRestartAction,
            style: _outlinedActionStyle(context),
            onPressed: busy || onStartOperation == null
                ? null
                : () => onStartOperation!(RuntimeOperationKind.restart),
            icon: const Icon(Icons.restart_alt),
            label: Text(localizations.runtimeRestartAction),
          ),
        if (state is RuntimeStatusOperating)
          OutlinedButton.icon(
            key: AppKeys.runtimeCancelAction,
            style: _outlinedActionStyle(context),
            onPressed: onCancelOperation,
            icon: const Icon(Icons.stop_circle_outlined),
            label: Text(localizations.runtimeCancelAction),
          ),
        if (canListModels || inventoryState is RuntimeInventoryLoaded)
          OutlinedButton.icon(
            key: AppKeys.runtimeModelsAction,
            style: _outlinedActionStyle(context),
            onPressed: busy ? null : onToggleModels,
            icon: const Icon(Icons.inventory_2_outlined),
            label: Text(
              inventoryState is RuntimeInventoryLoaded
                  ? localizations.runtimeModelsHideAction
                  : localizations.runtimeModelsAction,
            ),
          ),
      ],
    );
  }

  Future<void> _confirmReuse(BuildContext context) async {
    final localizations = AppLocalizations.of(context);
    final approved = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(localizations.runtimeConsentPromptTitle),
        content: Text(localizations.runtimeConsentPromptMessage),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(localizations.runtimeConsentCancelAction),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(localizations.runtimeConsentApproveAction),
          ),
        ],
      ),
    );
    if (approved == true) {
      onApproveReuse?.call();
    }
  }
}

class _RuntimeDetails extends StatelessWidget {
  const _RuntimeDetails({required this.report});

  final RuntimeHealthReport report;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final version =
        report.version?.reportedVersion ?? localizations.unknownValue;
    final endpoint = _endpointLabel(localizations, report.endpointSafety);
    return Semantics(
      key: AppKeys.runtimeDetails,
      container: true,
      label: localizations.runtimeDetailsSemanticLabel(
        report.displayName,
        version,
        endpoint,
        _ownershipLabel(localizations, report.ownership),
        _consentLabel(localizations, report.reuseConsent),
        _managementConsentLabel(localizations, report.managementConsent),
      ),
      child: ExcludeSemantics(
        child: Wrap(
          spacing: 32,
          runSpacing: 16,
          children: [
            _Detail(
              label: localizations.runtimeProviderLabel,
              value: report.displayName,
            ),
            _Detail(label: localizations.runtimeVersionLabel, value: version),
            _Detail(
              label: localizations.runtimeOwnershipLabel,
              value: _ownershipLabel(localizations, report.ownership),
            ),
            _Detail(
              label: localizations.runtimeReuseConsentLabel,
              value: _consentLabel(localizations, report.reuseConsent),
            ),
            _Detail(
              label: localizations.runtimeManagementConsentLabel,
              value: _managementConsentLabel(
                localizations,
                report.managementConsent,
              ),
            ),
            _Detail(label: localizations.runtimeEndpointLabel, value: endpoint),
          ],
        ),
      ),
    );
  }
}

class _RuntimeInventory extends StatelessWidget {
  const _RuntimeInventory({required this.state});

  final RuntimeInventoryState state;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    return Column(
      key: AppKeys.runtimeModels,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          localizations.runtimeModelsTitle,
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 12),
        switch (state) {
          RuntimeInventoryLoading() => Semantics(
            key: AppKeys.runtimeModelsProgress,
            label: localizations.runtimeModelsProgressSemanticLabel,
            value: localizations.inProgressSemanticValue,
            child: const LinearProgressIndicator(),
          ),
          RuntimeInventoryLoaded(:final inventory)
              when inventory.models.isEmpty =>
            Text(localizations.runtimeModelsEmpty),
          RuntimeInventoryLoaded(:final inventory) => Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (var index = 0; index < inventory.models.length; index++) ...[
                _RuntimeModel(model: inventory.models[index]),
                if (index < inventory.models.length - 1)
                  const Divider(height: 24),
              ],
              if (inventory.truncated) ...[
                const SizedBox(height: 12),
                Text(localizations.runtimeModelsTruncated),
              ],
            ],
          ),
          RuntimeInventoryFailed() => Text(
            localizations.runtimeModelsFailedMessage,
          ),
          RuntimeInventoryIdle() => const SizedBox.shrink(),
        },
      ],
    );
  }
}

class _RuntimeModel extends StatelessWidget {
  const _RuntimeModel({required this.model});

  final RuntimeModelSummary model;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final catalogueId = model.mapping.catalogueId;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(model.displayName, style: Theme.of(context).textTheme.bodyLarge),
        const SizedBox(height: 4),
        Text(
          catalogueId == null
              ? localizations.runtimeExternalModelLabel
              : localizations.runtimeModelCatalogueLabel(catalogueId),
          style: Theme.of(context).textTheme.bodySmall,
        ),
        if (model.sizeBytes case final size?) ...[
          const SizedBox(height: 4),
          Text(
            localizations.runtimeModelSizeLabel(_formatBytes(size)),
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ],
      ],
    );
  }
}

class _MessageList extends StatelessWidget {
  const _MessageList({
    required this.title,
    required this.messages,
    required this.icon,
  });

  final String title;
  final Iterable<String> messages;
  final IconData icon;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(title, style: Theme.of(context).textTheme.labelLarge),
        const SizedBox(height: 6),
        for (final message in messages)
          Padding(
            padding: const EdgeInsets.only(bottom: 6),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Icon(icon, size: 18),
                const SizedBox(width: 8),
                Expanded(child: Text(message)),
              ],
            ),
          ),
      ],
    );
  }
}

class _Detail extends StatelessWidget {
  const _Detail({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 180,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: Theme.of(context).textTheme.labelLarge),
          const SizedBox(height: 4),
          Text(value),
        ],
      ),
    );
  }
}

RuntimeHealthReport? _reportFor(RuntimeStatusState state) {
  return switch (state) {
    RuntimeStatusLoading(:final previousReport) => previousReport,
    RuntimeStatusLoaded(:final report) ||
    RuntimeStatusOperating(:final report) => report,
    RuntimeStatusFailed(:final report) ||
    RuntimeStatusCancelled(:final report) => report,
    RuntimeStatusIdle() => null,
  };
}

_RuntimeStatus _statusFor(
  AppLocalizations localizations,
  RuntimeStatusState state,
) {
  return switch (state) {
    RuntimeStatusIdle() => _RuntimeStatus(
      title: localizations.runtimeIdleTitle,
      message: localizations.runtimeIdleMessage,
      icon: Icons.power_settings_new,
      tone: _RuntimeTone.neutral,
    ),
    RuntimeStatusLoading() => _RuntimeStatus(
      title: localizations.runtimeCheckingTitle,
      message: localizations.runtimeCheckingMessage,
      icon: Icons.sync,
      tone: _RuntimeTone.neutral,
    ),
    RuntimeStatusLoaded(:final report) => _statusForReport(
      localizations,
      report,
    ),
    RuntimeStatusOperating() => _RuntimeStatus(
      title: localizations.runtimeOperationTitle,
      message: localizations.runtimeOperationMessage,
      icon: Icons.sync,
      tone: _RuntimeTone.neutral,
    ),
    RuntimeStatusFailed(:final operationKind) =>
      operationKind == null
          ? _RuntimeStatus(
              title: localizations.runtimeFailedTitle,
              message: localizations.runtimeFailedMessage,
              icon: Icons.error_outline,
              tone: _RuntimeTone.error,
            )
          : _RuntimeStatus(
              title: localizations.runtimeOperationFailedTitle(
                _operationLabel(localizations, operationKind),
              ),
              message: localizations.runtimeOperationFailedMessage(
                _operationLabel(localizations, operationKind),
              ),
              icon: Icons.error_outline,
              tone: _RuntimeTone.error,
            ),
    RuntimeStatusCancelled() => _RuntimeStatus(
      title: localizations.runtimeCancelledTitle,
      message: localizations.runtimeCancelledMessage,
      icon: Icons.cancel_outlined,
      tone: _RuntimeTone.neutral,
    ),
  };
}

_RuntimeStatus _statusForReport(
  AppLocalizations localizations,
  RuntimeHealthReport report,
) {
  return switch (report.state) {
    RuntimeState.notInstalled => _RuntimeStatus(
      title: localizations.runtimeNotInstalledTitle,
      message: localizations.runtimeNotInstalledMessage,
      icon: Icons.extension_off_outlined,
      tone: _RuntimeTone.warning,
    ),
    RuntimeState.installedStopped => _RuntimeStatus(
      title: localizations.runtimeInstalledStoppedTitle,
      message: localizations.runtimeInstalledStoppedMessage,
      icon: Icons.stop_circle_outlined,
      tone: _RuntimeTone.warning,
    ),
    RuntimeState.starting => _RuntimeStatus(
      title: localizations.runtimeStartingTitle,
      message: localizations.runtimeStartingMessage,
      icon: Icons.sync,
      tone: _RuntimeTone.neutral,
    ),
    RuntimeState.ready => _RuntimeStatus(
      title: localizations.runtimeReadyTitle,
      message: localizations.runtimeReadyMessage,
      icon: Icons.check_circle_outline,
      tone: _RuntimeTone.ready,
    ),
    RuntimeState.degraded => _RuntimeStatus(
      title: localizations.runtimeDegradedTitle,
      message: localizations.runtimeDegradedMessage,
      icon: Icons.warning_amber,
      tone: _RuntimeTone.warning,
    ),
    RuntimeState.incompatible => _RuntimeStatus(
      title: localizations.runtimeIncompatibleTitle,
      message: localizations.runtimeIncompatibleMessage,
      icon: Icons.system_update_alt,
      tone: _RuntimeTone.error,
    ),
    RuntimeState.updating => _RuntimeStatus(
      title: localizations.runtimeUpdatingTitle,
      message: localizations.runtimeUpdatingMessage,
      icon: Icons.system_update,
      tone: _RuntimeTone.neutral,
    ),
    RuntimeState.failed => _RuntimeStatus(
      title: localizations.runtimeFailedTitle,
      message: localizations.runtimeFailedMessage,
      icon: Icons.error_outline,
      tone: _RuntimeTone.error,
    ),
    RuntimeState.unknown => _RuntimeStatus(
      title: localizations.runtimeUnknownTitle,
      message: localizations.runtimeUnknownMessage,
      icon: Icons.help_outline,
      tone: _RuntimeTone.warning,
    ),
  };
}

String _ownershipLabel(
  AppLocalizations localizations,
  RuntimeOwnership? ownership,
) {
  return switch (ownership) {
    RuntimeOwnership.external => localizations.runtimeOwnershipExternal,
    RuntimeOwnership.gixGizManaged => localizations.runtimeOwnershipManaged,
    RuntimeOwnership.bundled => localizations.runtimeOwnershipBundled,
    RuntimeOwnership.unknown || null => localizations.runtimeOwnershipUnknown,
  };
}

String _consentLabel(
  AppLocalizations localizations,
  RuntimeConsentState? consent,
) {
  return switch (consent) {
    RuntimeConsentState.notRequested =>
      localizations.runtimeConsentNotRequested,
    RuntimeConsentState.reuseApproved =>
      localizations.runtimeConsentReuseApproved,
    RuntimeConsentState.managementApproved =>
      localizations.runtimeConsentManagementApproved,
    RuntimeConsentState.denied => localizations.runtimeConsentDenied,
    RuntimeConsentState.unknown || null => localizations.runtimeConsentUnknown,
  };
}

String _managementConsentLabel(
  AppLocalizations localizations,
  RuntimeConsentState? consent,
) {
  return switch (consent) {
    RuntimeConsentState.managementApproved =>
      localizations.runtimeManagementConsentApproved,
    RuntimeConsentState.denied => localizations.runtimeManagementConsentDenied,
    RuntimeConsentState.notRequested || RuntimeConsentState.reuseApproved =>
      localizations.runtimeManagementConsentNotRequested,
    RuntimeConsentState.unknown ||
    null => localizations.runtimeManagementConsentUnknown,
  };
}

String _operationLabel(
  AppLocalizations localizations,
  RuntimeOperationKind kind,
) {
  return switch (kind) {
    RuntimeOperationKind.start => localizations.runtimeOperationKindStart,
    RuntimeOperationKind.stop => localizations.runtimeOperationKindStop,
    RuntimeOperationKind.restart => localizations.runtimeOperationKindRestart,
    RuntimeOperationKind.unknown => localizations.runtimeOperationKindUnknown,
  };
}

String _recoveryActionLabel(
  AppLocalizations localizations,
  RecoveryAction action,
) {
  return switch (action) {
    RecoveryAction.retry => localizations.runtimeRecoveryRetry,
    RecoveryAction.restart => localizations.runtimeRecoveryRestart,
    RecoveryAction.checkPrerequisites =>
      localizations.runtimeRecoveryCheckPrerequisites,
    RecoveryAction.contactSupport =>
      localizations.runtimeRecoveryContactSupport,
    RecoveryAction.noAction => localizations.runtimeRecoveryNoAction,
    RecoveryAction.unknown => localizations.runtimeRecoveryUnknown,
  };
}

String _reasonLabel(AppLocalizations localizations, RuntimeReasonCode code) {
  return switch (code) {
    RuntimeReasonCode.installationNotFound =>
      localizations.runtimeReasonInstallationNotFound,
    RuntimeReasonCode.executableVerified =>
      localizations.runtimeReasonExecutableVerified,
    RuntimeReasonCode.endpointUnavailable =>
      localizations.runtimeReasonEndpointUnavailable,
    RuntimeReasonCode.endpointReachable =>
      localizations.runtimeReasonEndpointReachable,
    RuntimeReasonCode.endpointUnsafe =>
      localizations.runtimeReasonEndpointUnsafe,
    RuntimeReasonCode.versionCompatible =>
      localizations.runtimeReasonVersionCompatible,
    RuntimeReasonCode.versionIncompatible =>
      localizations.runtimeReasonVersionIncompatible,
    RuntimeReasonCode.versionUnverified =>
      localizations.runtimeReasonVersionUnverified,
    RuntimeReasonCode.evidenceIncomplete =>
      localizations.runtimeReasonEvidenceIncomplete,
    RuntimeReasonCode.ownershipRequired =>
      localizations.runtimeReasonOwnershipRequired,
    RuntimeReasonCode.consentRequired =>
      localizations.runtimeReasonConsentRequired,
    RuntimeReasonCode.operationUnsupported =>
      localizations.runtimeReasonOperationUnsupported,
    RuntimeReasonCode.processExited => localizations.runtimeReasonProcessExited,
    RuntimeReasonCode.unknown => localizations.runtimeReasonUnknown,
  };
}

String _warningLabel(AppLocalizations localizations, RuntimeWarningCode code) {
  return switch (code) {
    RuntimeWarningCode.externalInstallation =>
      localizations.runtimeWarningExternalInstallation,
    RuntimeWarningCode.endpointExposure =>
      localizations.runtimeWarningEndpointExposure,
    RuntimeWarningCode.versionUntested =>
      localizations.runtimeWarningVersionUntested,
    RuntimeWarningCode.partialEvidence =>
      localizations.runtimeWarningPartialEvidence,
    RuntimeWarningCode.modelInventoryTruncated =>
      localizations.runtimeWarningModelInventoryTruncated,
    RuntimeWarningCode.unknown => localizations.runtimeWarningUnknown,
  };
}

String _endpointLabel(
  AppLocalizations localizations,
  RuntimeEndpointSafety safety,
) {
  return switch (safety) {
    RuntimeEndpointSafety.loopbackVerified =>
      localizations.runtimeEndpointVerified,
    RuntimeEndpointSafety.unsafe => localizations.runtimeEndpointRejected,
    RuntimeEndpointSafety.unverified ||
    RuntimeEndpointSafety.unknown => localizations.runtimeEndpointUnknown,
  };
}

bool _isAvailable(RuntimeHealthReport? report, RuntimeCapabilityKind kind) {
  if (report == null) {
    return false;
  }
  return report.capabilities.any(
    (capability) =>
        capability.kind == kind &&
        capability.availability == RuntimeCapabilityAvailability.available,
  );
}

bool _hasAvailability(
  RuntimeHealthReport report,
  RuntimeCapabilityAvailability availability,
) {
  return report.capabilities.any(
    (capability) => capability.availability == availability,
  );
}

String _formatBytes(int bytes) {
  const gibibyte = 1024 * 1024 * 1024;
  if (bytes >= gibibyte) {
    return '${(bytes / gibibyte).toStringAsFixed(1)} GiB';
  }
  const mebibyte = 1024 * 1024;
  return '${(bytes / mebibyte).toStringAsFixed(1)} MiB';
}

ButtonStyle _outlinedActionStyle(BuildContext context) {
  final colorScheme = Theme.of(context).colorScheme;
  return ButtonStyle(
    side: WidgetStateProperty.resolveWith((states) {
      if (states.contains(WidgetState.focused)) {
        return BorderSide(color: colorScheme.onSurface, width: 2);
      }
      if (states.contains(WidgetState.disabled)) {
        return BorderSide(color: colorScheme.onSurface.withValues(alpha: 0.12));
      }
      return BorderSide(color: colorScheme.outline);
    }),
  );
}

enum _RuntimeTone { ready, warning, error, neutral }

class _RuntimeStatus {
  const _RuntimeStatus({
    required this.title,
    required this.message,
    required this.icon,
    required this.tone,
  });

  final String title;
  final String message;
  final IconData icon;
  final _RuntimeTone tone;
}
