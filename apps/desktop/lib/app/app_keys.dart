import 'package:flutter/widgets.dart';

abstract final class AppKeys {
  static const brand = ValueKey<String>('app.brand');
  static const foundationNavigation = ValueKey<String>('navigation.foundation');
  static const aboutNavigation = ValueKey<String>('navigation.about');
  static const foundationStatus = ValueKey<String>('foundation.status');
  static const foundationProgress = ValueKey<String>('foundation.progress');
  static const primaryAction = ValueKey<String>('foundation.primary_action');
  static const diagnostics = ValueKey<String>('foundation.diagnostics');
  static const coreDetails = ValueKey<String>('foundation.core_details');
  static const hardwareScan = ValueKey<String>('foundation.hardware_scan');
  static const hardwareScanStatus = ValueKey<String>(
    'foundation.hardware_scan.status',
  );
  static const hardwareScanProgress = ValueKey<String>(
    'foundation.hardware_scan.progress',
  );
  static const hardwareScanPrimaryAction = ValueKey<String>(
    'foundation.hardware_scan.primary_action',
  );
  static const hardwareScanCancelAction = ValueKey<String>(
    'foundation.hardware_scan.cancel_action',
  );
  static const hardwareScanDiagnostics = ValueKey<String>(
    'foundation.hardware_scan.diagnostics',
  );
  static const capabilityRecommendation = ValueKey<String>(
    'foundation.capability',
  );
  static const capabilityWorkload = ValueKey<String>(
    'foundation.capability.workload',
  );
  static const capabilityLargerOption = ValueKey<String>(
    'foundation.capability.larger_option',
  );
  static const capabilityGenerateAction = ValueKey<String>(
    'foundation.capability.generate_action',
  );
  static const capabilityProgress = ValueKey<String>(
    'foundation.capability.progress',
  );
  static const capabilityStatus = ValueKey<String>(
    'foundation.capability.status',
  );
  static const capabilityRecommendedPlan = ValueKey<String>(
    'foundation.capability.recommended',
  );
  static const capabilityFallbackPlan = ValueKey<String>(
    'foundation.capability.fallback',
  );
  static const capabilityLargerPlan = ValueKey<String>(
    'foundation.capability.larger',
  );
  static const capabilityNoPlan = ValueKey<String>(
    'foundation.capability.no_plan',
  );
  static const runtimePanel = ValueKey<String>('foundation.runtime');
  static const runtimeStatus = ValueKey<String>('foundation.runtime.status');
  static const runtimeDetails = ValueKey<String>('foundation.runtime.details');
  static const runtimeProgress = ValueKey<String>(
    'foundation.runtime.progress',
  );
  static const runtimeRefreshAction = ValueKey<String>(
    'foundation.runtime.refresh_action',
  );
  static const runtimeConsentAction = ValueKey<String>(
    'foundation.runtime.consent_action',
  );
  static const runtimeStartAction = ValueKey<String>(
    'foundation.runtime.start_action',
  );
  static const runtimeStopAction = ValueKey<String>(
    'foundation.runtime.stop_action',
  );
  static const runtimeRestartAction = ValueKey<String>(
    'foundation.runtime.restart_action',
  );
  static const runtimeCancelAction = ValueKey<String>(
    'foundation.runtime.cancel_action',
  );
  static const runtimeModelsAction = ValueKey<String>(
    'foundation.runtime.models_action',
  );
  static const runtimeModels = ValueKey<String>('foundation.runtime.models');
  static const runtimeModelsProgress = ValueKey<String>(
    'foundation.runtime.models_progress',
  );
  static const runtimeDiagnostics = ValueKey<String>(
    'foundation.runtime.diagnostics',
  );
}
