export function SkeletonGrid({ count = 8 }: { count?: number }) {
    return (
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-4 p-4">
            {Array.from({ length: count }).map((_, i) => (
                <div key={i} className="bg-telegram-surface rounded-xl border border-telegram-border overflow-hidden" style={{ aspectRatio: '4/3' }}>
                    <div className="w-full h-full bg-telegram-hover/50 animate-pulse" />
                </div>
            ))}
        </div>
    );
}

export function SkeletonList({ count = 6 }: { count?: number }) {
    return (
        <div className="space-y-1 p-4">
            {Array.from({ length: count }).map((_, i) => (
                <div key={i} className="grid grid-cols-[2rem_2fr_6rem_8rem] gap-4 items-center px-4 py-3">
                    <div className="w-5 h-5 bg-telegram-hover/50 rounded animate-pulse" />
                    <div className="h-4 bg-telegram-hover/50 rounded w-3/4 animate-pulse" />
                    <div className="h-3 bg-telegram-hover/50 rounded w-12 ml-auto animate-pulse" />
                    <div className="h-3 bg-telegram-hover/50 rounded w-16 ml-auto animate-pulse" />
                </div>
            ))}
        </div>
    );
}
