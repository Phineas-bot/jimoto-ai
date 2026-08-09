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
}
