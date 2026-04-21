def analyze_track(*args, **kwargs):
    from fourfour_analysis.analyze import analyze_track
    return analyze_track(*args, **kwargs)


def analyze_batch(*args, **kwargs):
    from fourfour_analysis.analyze import analyze_batch
    return analyze_batch(*args, **kwargs)


__all__ = ["analyze_track", "analyze_batch"]
