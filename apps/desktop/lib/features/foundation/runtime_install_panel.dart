import 'package:flutter/material.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';

/// Guided local-runtime installation.
///
/// The panel renders authoritative core state only. It never infers success
/// from progress, from a finished transfer, or from an installer exit: a
/// runtime is reported ready only when the core says so.
class RuntimeInstallPanel extends StatelessWidget {
  const RuntimeInstallPanel({
    super.key,
    this.plan,
    this.attention,
    this.job,
    this.busy = false,
    this.onReviewPlan,
    this.onApprove,
    this.onDeny,
    this.onStart,
    this.onRefresh,
  });

  final RuntimeInstallPlan? plan;
  final RuntimeInstallAttentionReason? attention;
  final RuntimeInstallJobSnapshot? job;
  final bool busy;
  final VoidCallback? onReviewPlan;
  final VoidCallback? onApprove;
  final VoidCallback? onDeny;
  final VoidCallback? onStart;
  final VoidCallback? onRefresh;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final theme = Theme.of(context);

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Semantics(
              header: true,
              child: Text(
                localizations.runtimeInstallTitle,
                style: theme.textTheme.titleMedium,
              ),
            ),
            const SizedBox(height: 8),
            Text(_statusMessage(localizations)),
            if (job?.progress != null) ...[
              const SizedBox(height: 12),
              _TransferProgress(progress: job!.progress!),
            ],
            if (plan != null) ...[
              const SizedBox(height: 12),
              _PlanReview(plan: plan!),
            ],
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: _actions(localizations),
            ),
          ],
        ),
      ),
    );
  }

  List<Widget> _actions(AppLocalizations localizations) {
    final snapshot = job;
    if (snapshot == null) {
      return [
        FilledButton(
          onPressed: busy ? null : onReviewPlan,
          child: Text(localizations.runtimeInstallReviewAction),
        ),
      ];
    }
    switch (snapshot.state) {
      case RuntimeInstallState.awaitingApproval:
        return [
          FilledButton(
            onPressed: busy ? null : onApprove,
            child: Text(localizations.runtimeInstallApproveAction),
          ),
          OutlinedButton(
            onPressed: busy ? null : onDeny,
            child: Text(localizations.runtimeInstallDenyAction),
          ),
        ];
      case RuntimeInstallState.approved:
        return [
          FilledButton(
            onPressed: busy ? null : onStart,
            child: Text(localizations.runtimeInstallStartAction),
          ),
        ];
      case RuntimeInstallState.running:
        return [
          OutlinedButton(
            onPressed: busy ? null : onRefresh,
            child: Text(localizations.runtimeInstallRefreshAction),
          ),
        ];
      case RuntimeInstallState.attentionRequired:
      case RuntimeInstallState.failed:
      case RuntimeInstallState.cancelled:
        return [
          FilledButton(
            onPressed: busy ? null : onReviewPlan,
            child: Text(localizations.runtimeInstallReviewAction),
          ),
        ];
      case RuntimeInstallState.ready:
      case RuntimeInstallState.unknown:
        return const <Widget>[];
    }
  }

  String _statusMessage(AppLocalizations localizations) {
    final snapshot = job;
    if (snapshot != null) {
      switch (snapshot.state) {
        case RuntimeInstallState.awaitingApproval:
          return localizations.runtimeInstallAwaitingApproval;
        case RuntimeInstallState.approved:
          return localizations.runtimeInstallApproved;
        case RuntimeInstallState.running:
          return localizations.runtimeInstallRunning;
        case RuntimeInstallState.ready:
          return localizations.runtimeInstallReady;
        case RuntimeInstallState.failed:
          return localizations.runtimeInstallFailed;
        case RuntimeInstallState.cancelled:
          return localizations.runtimeInstallCancelled;
        case RuntimeInstallState.attentionRequired:
        case RuntimeInstallState.unknown:
          return localizations.runtimeInstallAttention;
      }
    }
    if (attention == RuntimeInstallAttentionReason.externalRuntimePresent) {
      return localizations.runtimeInstallExternalPresent;
    }
    if (attention != null) {
      return localizations.runtimeInstallUnavailable;
    }
    return localizations.runtimeInstallAvailable;
  }
}

/// Shows exactly what approval authorizes, before anything is downloaded.
class _PlanReview extends StatelessWidget {
  const _PlanReview({required this.plan});

  final RuntimeInstallPlan plan;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final theme = Theme.of(context);
    final component = plan.components.isEmpty ? null : plan.components.first;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (component != null) ...[
          _Row(label: localizations.runtimeInstallComponent, value: component.displayName),
          _Row(label: localizations.runtimeInstallVersion, value: component.version),
          _Row(label: localizations.runtimeInstallSource, value: component.sourceOrigin),
          _Row(
            label: localizations.runtimeInstallPublisher,
            value: component.expectedPublisher,
          ),
        ],
        _Row(
          label: localizations.runtimeInstallDownloadSize,
          value: _megabytes(plan.expectedDownloadBytes),
        ),
        _Row(
          label: localizations.runtimeInstallAdministrator,
          value: plan.requiresAdministrator
              ? localizations.runtimeInstallAdministratorRequired
              : localizations.runtimeInstallAdministratorNotRequired,
        ),
        const SizedBox(height: 8),
        for (final warning in plan.warnings)
          Padding(
            padding: const EdgeInsets.only(bottom: 4),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Icon(Icons.info_outline, size: 16),
                const SizedBox(width: 6),
                Expanded(
                  child: Text(warning, style: theme.textTheme.bodySmall),
                ),
              ],
            ),
          ),
      ],
    );
  }

  String _megabytes(int bytes) {
    final megabytes = bytes / (1024 * 1024);
    return '${megabytes.toStringAsFixed(0)} MB';
  }
}

class _Row extends StatelessWidget {
  const _Row({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 160,
            child: Text(label, style: theme.textTheme.bodySmall),
          ),
          Expanded(child: Text(value, style: theme.textTheme.bodyMedium)),
        ],
      ),
    );
  }
}

/// Live transfer progress.
///
/// Shows measured bytes rather than an inferred completion state: reaching one
/// hundred percent means the bytes arrived, never that the runtime is ready.
class _TransferProgress extends StatelessWidget {
  const _TransferProgress({required this.progress});

  final RuntimeInstallProgress progress;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final theme = Theme.of(context);
    final expected = progress.expectedBytes;
    final fraction = (expected != null && expected > 0)
        ? (progress.transferredBytes / expected).clamp(0.0, 1.0)
        : null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        LinearProgressIndicator(value: fraction),
        const SizedBox(height: 6),
        Text(
          fraction == null
              ? localizations.runtimeInstallTransferredUnknownTotal(
                  _megabytes(progress.transferredBytes),
                )
              : localizations.runtimeInstallTransferred(
                  _megabytes(progress.transferredBytes),
                  _megabytes(expected!),
                  (fraction * 100).round(),
                ),
          style: theme.textTheme.bodySmall,
        ),
      ],
    );
  }

  String _megabytes(int bytes) =>
      '${(bytes / (1024 * 1024)).toStringAsFixed(0)} MB';
}
